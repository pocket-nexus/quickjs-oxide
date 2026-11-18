use alloc::{vec, vec::Vec};

use super::{
    HEADER_SIZE, ImageError, MAGIC, NONE, VERSION, Writer, fingerprint, keyed, packed, wire,
};
use crate::bytecode::{Capture, CodeUnit, flags};
use crate::heap::string::StringRepr;
use crate::{FxHashMap, Heap, KeyKind, Kind, ObjectId, ObjectKind, Runtime, StringId, Value};

/// Exports a freshly compiled classic script before it has executed.
/// Heap constants are encoded by value. Templates are rebuilt in the target
/// realm with its own array prototype.
pub fn encode_script(runtime: &Runtime, script: ObjectId) -> Result<Vec<u8>, ImageError> {
    let heap = runtime.heap();
    let entry = heap
        .function_bytecode(script)
        .ok_or(ImageError::UnsupportedScript)?;
    let entry_unit = runtime.units().get(heap.bytecode_unit(entry));
    if entry_unit.image.is_some()
        || !entry_unit.has(flags::SCRIPT)
        || entry_unit.has(flags::MODULE | flags::EVAL)
    {
        return Err(ImageError::UnsupportedScript);
    }

    let mut units = Vec::new();
    let mut pending = vec![entry];
    let mut seen = FxHashMap::default();
    while let Some(record) = pending.pop() {
        let id = heap.bytecode_unit(record);
        if seen.insert(id, ()).is_some() {
            return Err(ImageError::UnsupportedScript);
        }
        let unit = runtime.units().get(id);
        pending.extend((0..unit.children.len()).map(|i| heap.bytecode_child(record, i as u32)));
        units.push((id, record));
    }
    let ids: FxHashMap<_, _> = units
        .iter()
        .rev()
        .enumerate()
        .map(|(index, &(id, _))| (id, index as u32))
        .collect();

    let mut masks = [packed::NO_IMMEDIATE; packed::MASKS];
    for &(id, _) in &units {
        for insn in &runtime.units().get(id).code {
            let mask = &mut masks[insn.op as usize];
            *mask |= packed::mode(*insn);
            if insn.imm != 0 || keyed(insn.op) {
                *mask &= !packed::NO_IMMEDIATE;
            }
        }
    }
    let mut encoder = Encoder {
        heap,
        raw_key: runtime.realm().names.raw,
        strings: FxHashMap::default(),
        text: Vec::new(),
        units: Writer::default(),
    };
    encoder.units.count(units.len())?;
    for (id, record) in units.into_iter().rev() {
        let unit = runtime.units().get(id);
        if unit.debug.filename != entry_unit.debug.filename {
            return Err(ImageError::UnsupportedScript);
        }
        let children = unit.children.iter().map(|id| ids[id]);
        encoder.unit(&unit, &masks, children, heap.bytecode_constants(record))?;
    }

    let mut body = Writer::default();
    body.blob(
        entry_unit
            .debug
            .filename
            .as_deref()
            .unwrap_or("")
            .as_bytes(),
    )?;
    body.count(encoder.text.len())?;
    for id in encoder.text {
        match heap.string(id).repr() {
            StringRepr::OneByte(bytes) => {
                body.u8(0);
                body.blob(bytes)?;
            }
            StringRepr::TwoByte(units) => {
                body.u8(1);
                body.count(units.len())?;
                for &unit in units {
                    body.u16(unit);
                }
            }
        }
    }
    body.0.extend(masks);
    body.0.extend(encoder.units.0);

    let mut image = Writer(Vec::with_capacity(usize::from(HEADER_SIZE) + body.0.len()));
    image.0.extend(MAGIC);
    image.u16(VERSION);
    image.u16(HEADER_SIZE);
    image.u32(u32::try_from(body.0.len()).map_err(|_| ImageError::TooLarge)?);
    image.u64(fingerprint());
    image.u32(wire::checksum(&body.0));
    image.0.extend(body.0);
    Ok(image.0)
}

struct Encoder<'a> {
    heap: &'a Heap,
    raw_key: StringId,
    strings: FxHashMap<StringId, u32>,
    text: Vec<StringId>,
    units: Writer,
}

impl Encoder<'_> {
    fn text(&mut self, id: StringId) -> u32 {
        *self.strings.entry(id).or_insert_with(|| {
            let index = self.text.len() as u32;
            self.text.push(id);
            index
        })
    }

    fn unit(
        &mut self,
        unit: &CodeUnit,
        masks: &[u8; packed::MASKS],
        children: impl Iterator<Item = u32>,
        constants: &[Value],
    ) -> Result<(), ImageError> {
        for value in [
            unit.register_count,
            unit.parameter_count,
            unit.local_count,
            unit.flags,
            unit.this_slots,
            unit.spill_register,
            unit.spill_slots,
            unit.length,
        ] {
            self.units.var_u32(u32::from(value));
        }
        self.units.var_u32(unit.ic_count);
        let name = unit.name_atom.map_or(NONE, |id| self.text(id));
        self.units.var_u32(name.wrapping_add(1));
        self.units.blob(unit.debug.name.as_bytes())?;
        let (line, column) = unit.debug.definition.unwrap_or_default();
        self.units.var_u32(line);
        self.units.var_u32(column);
        self.units
            .blob(unit.source.as_deref().unwrap_or_default())?;

        let mut used = [0u8; 3];
        for insn in &unit.code {
            used[0] |= insn.a;
            used[1] |= insn.b;
            used[2] |= insn.c;
        }
        let widths = used.map(|bits| 8 - bits.leading_zeros() as u8);
        let mut instructions = Writer::default();
        packed::write(
            &mut instructions,
            masks,
            widths,
            unit.code.iter().copied().map(|mut insn| {
                if keyed(insn.op) {
                    insn.imm = match insn.key().kind() {
                        KeyKind::String(id) => self.text(id),
                        KeyKind::Index(_) => insn.imm,
                        KeyKind::Symbol(_) => return Err(ImageError::UnsupportedScript),
                    };
                }
                Ok(insn)
            }),
        )?;
        self.units.0.extend(instructions.0);
        self.units.count(unit.handlers.len())?;
        for handler in &unit.handlers {
            self.units.var_u32(handler.start);
            self.units.var_u32(handler.end);
            self.units.var_u32(handler.target);
            self.units.var_u32(u32::from(handler.register));
        }
        self.units.count(unit.children.len())?;
        for child in children {
            self.units.var_u32(child);
        }
        self.units.count(unit.captures.len())?;
        for capture in &unit.captures {
            let (tag, index) = match *capture {
                Capture::None => (0, 0),
                Capture::Register(index) => (1, index),
                Capture::Outer(index) => (2, index),
                Capture::Slot(index) => (3, index),
            };
            self.units.var_u32((u32::from(index) << 2) | tag);
        }
        self.units.count(unit.debug.positions.len())?;
        let mut positions = wire::Positions::new((line, column));
        for &position in &unit.debug.positions {
            positions.write(&mut self.units, position);
        }
        self.units.count(unit.direct_eval_sites.len())?;
        for &site in &unit.direct_eval_sites {
            self.units.var_u32(site);
        }
        self.units.count(constants.len())?;
        for &value in constants {
            self.constant(value)?;
        }
        Ok(())
    }

    fn constant(&mut self, value: Value) -> Result<(), ImageError> {
        let kind = value.kind();
        self.units.u8(match kind {
            Kind::Undefined => 0,
            Kind::Null => 1,
            Kind::Bool if value == Value::FALSE => 2,
            Kind::Bool => 3,
            Kind::Empty => 4,
            Kind::Int32 => 5,
            Kind::Number => 6,
            Kind::String => 7,
            Kind::BigInt => 8,
            Kind::Object => 9,
            Kind::Symbol => return Err(ImageError::UnsupportedScript),
        });
        match kind {
            Kind::Int32 => self.units.signed_delta(value.as_int32().unwrap() as u32),
            Kind::Number => self.units.u64(value.as_f64().unwrap().to_bits()),
            Kind::String => {
                let index = self.text(value.as_string().unwrap());
                self.units.var_u32(index);
            }
            Kind::BigInt => {
                let bigint = self.heap.bigint(value.as_bigint().unwrap());
                self.units.u8(u8::from(bigint.negative));
                self.units.count(bigint.limbs.len())?;
                for &limb in &bigint.limbs {
                    self.units.u64(limb);
                }
            }
            Kind::Object => self.template(value.as_object().unwrap())?,
            _ => {}
        }
        Ok(())
    }

    fn template(&mut self, cooked: ObjectId) -> Result<(), ImageError> {
        use crate::PropertyKey;
        let raw = self
            .heap
            .get_property(cooked, PropertyKey::atom(self.raw_key))
            .and_then(Value::as_object)
            .ok_or(ImageError::UnsupportedScript)?;
        if self.heap.object_kind(cooked) != ObjectKind::Array
            || self.heap.object_kind(raw) != ObjectKind::Array
            || self.heap.array_length(cooked) != self.heap.array_length(raw)
        {
            return Err(ImageError::UnsupportedScript);
        }
        let len = self.heap.array_length(raw);
        self.units.var_u32(len);
        for index in 0..len {
            let key = PropertyKey::index(index).ok_or(ImageError::UnsupportedScript)?;
            let cooked_value = self
                .heap
                .get_property(cooked, key)
                .ok_or(ImageError::UnsupportedScript)?;
            let cooked_index = if cooked_value.is_undefined() {
                NONE
            } else {
                self.text(
                    cooked_value
                        .as_string()
                        .ok_or(ImageError::UnsupportedScript)?,
                )
            };
            let raw_id = self
                .heap
                .get_property(raw, key)
                .and_then(Value::as_string)
                .ok_or(ImageError::UnsupportedScript)?;
            let raw_index = self.text(raw_id);
            self.units.u32(cooked_index);
            self.units.u32(raw_index);
        }
        Ok(())
    }
}
