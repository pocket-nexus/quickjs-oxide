use alloc::{boxed::Box, rc::Rc, vec::Vec};

use super::{
    Constant, HEADER_SIZE, ImageBytes, ImageCode, ImageError, ImageUnitData, LoadError, MAGIC,
    NONE, Reader, Text, VERSION, fingerprint, packed, text_ref, wire, wire_atom,
};
use crate::bytecode::{Capture, CodeUnit, CodeUnitId, DebugInfo, Handler, flags, validate};
use crate::heap::atom::KEY_INDEX_LIMIT;
use crate::vm::compile::Compiler;
use crate::{BytecodeTemplate, HeapError, Intrinsic, ObjectId, Runtime, StringId, Value};

/// A verified image of one classic script. Decoding requires neither a
/// runtime nor a source compiler. Installation interns literals and keys in
/// the destination realm and returns the script function, without running it.
#[derive(Clone)]
pub struct ScriptImage {
    bytes: ImageBytes,
    masks: core::ops::Range<usize>,
    text: Vec<Text>,
    units: Vec<CodeUnit>,
    unit_data: Vec<ImageUnitData>,
}

impl ScriptImage {
    /// Copies the bytes into immutable image storage. Use `decode_shared`
    /// when the package already has a shareable owner.
    pub fn decode(bytes: &[u8]) -> Result<Self, ImageError> {
        Self::decode_shared(Box::<[u8]>::from(bytes).into())
    }

    /// Verify once. The result retains the exact storage that was checked
    /// and can be passed from the package installer to the runtime.
    pub fn decode_shared(storage: ImageBytes) -> Result<Self, ImageError> {
        let bytes = storage.as_ref();
        let mut reader = Reader(bytes);
        if reader.take(4)? != MAGIC {
            return Err(ImageError::Format);
        }
        let version = reader.u16()?;
        if version != VERSION {
            return Err(ImageError::Version(version));
        }
        if reader.u16()? != HEADER_SIZE {
            return Err(ImageError::Format);
        }
        let length = reader.u32()? as usize;
        if reader.u64()? != fingerprint() {
            return Err(ImageError::Fingerprint);
        }
        let checksum = reader.u32()?;
        if reader.0.len() != length {
            return Err(ImageError::Malformed);
        }
        if wire::checksum(reader.0) != checksum {
            return Err(ImageError::Checksum);
        }

        let filename = reader.text()?;
        let filename = (!filename.is_empty()).then(|| Rc::<str>::from(filename));
        let text = reader.list(2, |reader| match reader.u8()? {
            0 => Ok(Text::Latin1(packed::span(bytes, reader.blob()?))),
            1 => {
                let count = reader.count(2)?;
                Ok(Text::Utf16(packed::span(bytes, reader.take(count * 2)?)))
            }
            _ => Err(ImageError::Malformed),
        })?;
        if text.len() > KEY_INDEX_LIMIT as usize {
            return Err(ImageError::TooLarge);
        }

        let masks = reader.take(packed::MASKS)?;
        if masks.iter().any(|&mask| mask > 15) {
            return Err(ImageError::Malformed);
        }
        let mask_range = packed::span(bytes, masks);

        // A tree in postorder. Every old unit is consumed by exactly one
        // parent, so instantiation visits each unit once and cannot cycle.
        let count = reader.count(24)?;
        let mut units = Vec::with_capacity(count);
        let mut unit_data = Vec::with_capacity(count);
        let mut has_parent = Vec::with_capacity(count);
        for index in 0..count {
            let (unit, data) = read_unit(bytes, &mut reader, text.len(), filename.clone())?;
            let root = index + 1 == count;
            if unit.has(flags::SCRIPT) != root
                || unit.has(flags::MODULE | flags::EVAL)
                || (root
                    && unit
                        .captures
                        .iter()
                        .any(|capture| *capture != Capture::None))
            {
                return Err(ImageError::Malformed);
            }
            for child in &unit.children {
                let used = has_parent
                    .get_mut(child.index() as usize)
                    .ok_or(ImageError::Malformed)?;
                if core::mem::replace(used, true) {
                    return Err(ImageError::Malformed);
                }
            }
            let code = data.packed.view(bytes, masks);
            validate::validate_stream(
                &unit,
                code.len(),
                |pc| code.get(pc),
                Some(text.len() as u32),
                &|id| units.get(id.index() as usize),
            )
            .map_err(ImageError::InvalidCode)?;
            has_parent.push(false);
            units.push(unit);
            unit_data.push(data);
        }
        if !reader.0.is_empty() || count == 0 || has_parent[..count - 1].contains(&false) {
            return Err(ImageError::Malformed);
        }
        Ok(Self {
            bytes: storage,
            masks: mask_range,
            text,
            units,
            unit_data,
        })
    }

    /// The exact immutable image checked during admission.
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_ref()
    }

    /// Takes the decoded image. The returned object is unrooted, like
    /// [Runtime::instantiate]. Run it or hold it before the next allocation.
    pub fn instantiate(self, runtime: &mut Runtime) -> Result<ObjectId, HeapError> {
        let policy = runtime.image_decode_policy();
        let mut compiler = runtime.compiler();
        let mark = compiler.heap_mut().roots().mark();
        let result = self.install(&mut compiler, policy);
        compiler.heap_mut().roots().truncate(mark);
        result
    }

    fn install(
        self,
        compiler: &mut Compiler<'_>,
        policy: super::ImageDecodePolicy,
    ) -> Result<ObjectId, HeapError> {
        let mut wide = Vec::new();
        let bytes = self.bytes.as_ref();
        let atoms: Rc<[StringId]> = self
            .text
            .into_iter()
            .map(|text| match text {
                Text::Latin1(range) => compiler.heap_mut().intern_latin1(&bytes[range]),
                Text::Utf16(range) => {
                    wide.clear();
                    wide.extend(
                        bytes[range]
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .copied()
                            .map(u16::from_le_bytes),
                    );
                    compiler.heap_mut().intern_units(&wide)
                }
            })
            .collect::<Result<Vec<_>, _>>()?
            .into();
        drop(wide);

        let first = compiler.reserve_image(self.units.len())?;
        let last = (self.units.len() - 1) as u32;
        let backing = Rc::new(packed::ImageBacking {
            bytes: self.bytes,
            masks: self.masks,
            atoms,
        });
        let mut units = self.units;
        let mut literals = Vec::with_capacity(units.len());
        for (
            code,
            ImageUnitData {
                constants,
                packed,
                positions,
            },
        ) in units.iter_mut().zip(self.unit_data)
        {
            let table = install_constants(compiler, &backing.atoms, constants)?;
            let image = ImageCode {
                backing: backing.clone(),
                packed,
                positions,
                cache_base: Default::default(),
                decoded: Default::default(),
                heat: Default::default(),
            };
            match policy {
                super::ImageDecodePolicy::Eager => {
                    image.decode()?;
                    image.heat.set(packed::Heat::Seen);
                }
                super::ImageDecodePolicy::Packed => image.heat.set(packed::Heat::KeepPacked),
                super::ImageDecodePolicy::Adaptive { .. } => {}
            }
            code.image = Some(Box::new(image));
            code.name_atom = code.name_atom.map(|id| backing.atoms[id.0 as usize]);
            let children: Vec<_> = code
                .children
                .iter()
                .map(|child| last - child.index())
                .collect();
            for child in &mut code.children {
                *child = CodeUnitId(first.index() + child.index());
            }
            literals.push((table, children));
        }
        // The bytecode allocator takes the root first and explicit child
        // indexes. Reversing postorder supplies both without rebuilding the
        // tree by traversing the published code store.
        let templates: Vec<_> = literals
            .iter()
            .enumerate()
            .rev()
            .map(|(index, (constants, children))| BytecodeTemplate {
                unit: CodeUnitId(first.index() + index as u32),
                constants,
                children,
            })
            .collect();
        let prototype = compiler.intrinsic(Intrinsic::FunctionPrototype);
        let script =
            compiler
                .heap_mut()
                .alloc_bytecode_function(Some(prototype), None, &templates)?;
        compiler.attach_image(first, units);
        Ok(script)
    }
}

impl Runtime {
    /// Load the image from a range in an immutable package allocation.
    pub fn load_image_shared(&mut self, bytes: ImageBytes) -> Result<ObjectId, LoadError> {
        ScriptImage::decode_shared(bytes)
            .map_err(LoadError::Rejected)?
            .instantiate(self)
            .map_err(LoadError::Heap)
    }

    /// Decode and instantiate one classic script image. Format rejection and
    /// allocation failure are distinct. The caller runs the script separately.
    pub fn load_image(&mut self, bytes: &[u8]) -> Result<ObjectId, LoadError> {
        ScriptImage::decode(bytes)
            .map_err(LoadError::Rejected)?
            .instantiate(self)
            .map_err(LoadError::Heap)
    }
}

fn read_unit(
    bytes: &[u8],
    reader: &mut Reader<'_>,
    strings: usize,
    filename: Option<Rc<str>>,
) -> Result<(CodeUnit, ImageUnitData), ImageError> {
    let register_count = u16::try_from(reader.var_u32()?).map_err(|_| ImageError::Malformed)?;
    let parameter_count = u16::try_from(reader.var_u32()?).map_err(|_| ImageError::Malformed)?;
    let local_count = u16::try_from(reader.var_u32()?).map_err(|_| ImageError::Malformed)?;
    let flags = u16::try_from(reader.var_u32()?).map_err(|_| ImageError::Malformed)?;
    let this_slots = u16::try_from(reader.var_u32()?).map_err(|_| ImageError::Malformed)?;
    let spill_register = u16::try_from(reader.var_u32()?).map_err(|_| ImageError::Malformed)?;
    let spill_slots = u16::try_from(reader.var_u32()?).map_err(|_| ImageError::Malformed)?;
    let length = u16::try_from(reader.var_u32()?).map_err(|_| ImageError::Malformed)?;
    let ic_count = reader.var_u32()?;
    let name_atom = match reader.var_u32()?.wrapping_sub(1) {
        NONE => None,
        index => Some(wire_atom(text_ref(index, strings)?)),
    };
    let name = Box::from(reader.text()?);
    let line = reader.var_u32()?;
    let column = reader.var_u32()?;
    let source = reader.blob()?;
    let source = (!source.is_empty()).then(|| Box::from(source));

    let packed = packed::PackedRanges::of(bytes, reader)?;
    let insn_count = packed.len();
    if ic_count as usize > insn_count || flags & !((flags::EVAL << 1) - 1) != 0 {
        return Err(ImageError::Malformed);
    }
    let handlers = reader.list(4, |reader| {
        Ok(Handler {
            start: reader.var_u32()?,
            end: reader.var_u32()?,
            target: reader.var_u32()?,
            register: u16::try_from(reader.var_u32()?).map_err(|_| ImageError::Malformed)?,
        })
    })?;
    let children = reader.list(1, |reader| Ok(CodeUnitId(reader.var_u32()?)))?;
    let captures = reader.list(1, |reader| {
        let encoded = reader.var_u32()?;
        let tag = encoded & 3;
        let index = u16::try_from(encoded >> 2).map_err(|_| ImageError::Malformed)?;
        match tag {
            0 if index == 0 => Ok(Capture::None),
            1 => Ok(Capture::Register(index)),
            2 => Ok(Capture::Outer(index)),
            3 => Ok(Capture::Slot(index)),
            _ => Err(ImageError::Malformed),
        }
    })?;
    let mut state = wire::Positions::new((line, column));
    let count = reader.count(1)?;
    let before = reader.0;
    let mut last = None;
    for _ in 0..count {
        let position = state.next(reader)?;
        if position.pc as usize >= insn_count || last.is_some_and(|pc| pc >= position.pc) {
            return Err(ImageError::Malformed);
        }
        last = Some(position.pc);
    }
    let positions = packed::span(bytes, &before[..before.len() - reader.0.len()]);
    let direct_eval_sites = reader.list(1, Reader::var_u32)?;
    if !direct_eval_sites.is_sorted()
        || direct_eval_sites
            .last()
            .is_some_and(|&pc| pc as usize >= insn_count)
    {
        return Err(ImageError::Malformed);
    }
    let constants = reader.list(1, |reader| read_constant(reader, strings))?;
    let unit = CodeUnit {
        code: Box::new([]),
        image: None,
        handlers: handlers.into(),
        register_count,
        parameter_count,
        local_count,
        flags,
        this_slots,
        constant_count: constants.len() as u32,
        spill_register,
        spill_slots,
        ic_base: 0,
        ic_count,
        children: children.into(),
        captures: captures.into(),
        debug: DebugInfo {
            name,
            filename,
            positions: Box::new([]),
            definition: (line != 0 || column != 0).then_some((line, column)),
        },
        name_atom,
        length,
        source,
        direct_eval_sites: direct_eval_sites.into(),
        atoms: Box::new([]),
    };
    Ok((
        unit,
        ImageUnitData {
            constants,
            packed,
            positions,
        },
    ))
}

fn read_constant(reader: &mut Reader<'_>, strings: usize) -> Result<Constant, ImageError> {
    Ok(Constant::Scalar(match reader.u8()? {
        0 => Value::UNDEFINED,
        1 => Value::NULL,
        2 => Value::FALSE,
        3 => Value::TRUE,
        4 => Value::EMPTY,
        5 => Value::int32(reader.signed_delta()? as i32),
        6 => Value::number(f64::from_bits(reader.u64()?)),
        7 => return Ok(Constant::String(text_ref(reader.var_u32()?, strings)?)),
        8 => {
            let sign = reader.u8()?;
            let limbs = reader.list(8, Reader::u64)?;
            if sign > 1 || (sign == 1 && limbs.is_empty()) || limbs.last() == Some(&0) {
                return Err(ImageError::Malformed);
            }
            return Ok(Constant::BigInt {
                negative: sign == 1,
                limbs: limbs.into(),
            });
        }
        9 => {
            let entries = reader.list(8, |reader| {
                let cooked = reader.u32()?;
                if cooked != NONE {
                    text_ref(cooked, strings)?;
                }
                let raw = text_ref(reader.u32()?, strings)?;
                Ok((cooked, raw))
            })?;
            return Ok(Constant::Template(entries.into()));
        }
        _ => return Err(ImageError::Malformed),
    }))
}

fn install_constants(
    compiler: &mut Compiler<'_>,
    atoms: &[StringId],
    constants: Vec<Constant>,
) -> Result<Vec<Value>, HeapError> {
    let string = |index: u32| Value::string(atoms[index as usize]);
    constants
        .into_iter()
        .map(|constant| {
            let value = match constant {
                Constant::Scalar(value) => return Ok(value),
                Constant::String(index) => return Ok(string(index)),
                Constant::BigInt { negative, limbs } => {
                    Value::bigint(compiler.heap_mut().alloc_bigint(negative, &limbs)?)
                }
                Constant::Template(entries) => {
                    let cooked: Vec<_> = entries
                        .iter()
                        .map(|&(index, _)| {
                            if index == NONE {
                                Value::UNDEFINED
                            } else {
                                string(index)
                            }
                        })
                        .collect();
                    let raw: Vec<_> = entries.iter().map(|&(_, index)| string(index)).collect();
                    Value::object(compiler.template_object(&cooked, &raw)?)
                }
            };
            compiler.heap_mut().roots().push(value);
            Ok(value)
        })
        .collect()
}
