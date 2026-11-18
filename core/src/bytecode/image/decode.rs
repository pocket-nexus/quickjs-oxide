use alloc::{boxed::Box, rc::Rc, vec::Vec};

use super::{
    Constant, HEADER_SIZE, ImageError, ImageUnit, LoadError, MAGIC, NONE, Reader, Text, VERSION,
    fingerprint, text_ref, wire, wire_atom,
};
use crate::bytecode::{
    Capture, CodeUnit, CodeUnitId, DebugInfo, Format, Handler, Insn, Opcode, SourcePosition, flags,
    validate,
};
use crate::heap::atom::KEY_INDEX_LIMIT;
use crate::vm::compile::Compiler;
use crate::{
    BytecodeTemplate, HeapError, Intrinsic, KeyKind, ObjectId, PropertyKey, Runtime, StringId,
    Value,
};

/// A verified image of one classic script. Decoding requires neither a
/// runtime nor a source compiler. Installation interns literals and keys in
/// the destination realm and returns the script function, without running it.
pub struct ScriptImage<'a> {
    text: Vec<Text<'a>>,
    units: Vec<ImageUnit>,
}

impl<'a> ScriptImage<'a> {
    pub fn decode(bytes: &'a [u8]) -> Result<Self, ImageError> {
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
        let text = reader.list(5, |reader| match reader.u8()? {
            0 => Ok(Text::Latin1(reader.blob()?)),
            1 => {
                let count = reader.count(2)?;
                Ok(Text::Utf16(reader.take(count * 2)?))
            }
            _ => Err(ImageError::Malformed),
        })?;
        if text.len() > KEY_INDEX_LIMIT as usize {
            return Err(ImageError::TooLarge);
        }

        // A tree in postorder. Every old unit is consumed by exactly one
        // parent, so instantiation visits each unit once and cannot cycle.
        let count = reader.count(64)?;
        let mut units: Vec<ImageUnit> = Vec::with_capacity(count);
        let mut has_parent = Vec::with_capacity(count);
        for index in 0..count {
            let unit = read_unit(&mut reader, text.len(), filename.clone())?;
            let root = index + 1 == count;
            if unit.code.has(flags::SCRIPT) != root
                || unit.code.has(flags::MODULE | flags::EVAL)
                || (root
                    && unit
                        .code
                        .captures
                        .iter()
                        .any(|capture| *capture != Capture::None))
            {
                return Err(ImageError::Malformed);
            }
            for child in &unit.code.children {
                let used = has_parent
                    .get_mut(child.index() as usize)
                    .ok_or(ImageError::Malformed)?;
                if core::mem::replace(used, true) {
                    return Err(ImageError::Malformed);
                }
            }
            validate::validate(&unit.code, &|id| {
                units.get(id.index() as usize).map(|unit| &unit.code)
            })
            .map_err(ImageError::InvalidCode)?;
            has_parent.push(false);
            units.push(unit);
        }
        if !reader.0.is_empty() || count == 0 || has_parent[..count - 1].contains(&false) {
            return Err(ImageError::Malformed);
        }
        Ok(Self { text, units })
    }

    /// Takes the decoded image. The returned object is unrooted, like
    /// [Runtime::instantiate]. Run it or hold it before the next allocation.
    pub fn instantiate(self, runtime: &mut Runtime) -> Result<ObjectId, HeapError> {
        let mut compiler = runtime.compiler();
        let mark = compiler.heap_mut().roots().mark();
        let result = self.install(&mut compiler);
        compiler.heap_mut().roots().truncate(mark);
        result
    }

    fn install(self, compiler: &mut Compiler<'_>) -> Result<ObjectId, HeapError> {
        let mut wide = Vec::new();
        let atoms = self
            .text
            .into_iter()
            .map(|text| match text {
                Text::Latin1(bytes) => compiler.heap_mut().intern_latin1(bytes),
                Text::Utf16(bytes) => {
                    wide.clear();
                    wide.extend(
                        bytes
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .copied()
                            .map(u16::from_le_bytes),
                    );
                    compiler.heap_mut().intern_units(&wide)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        drop(wide);

        let last = (self.units.len() - 1) as u32;
        let mut linked: Vec<(CodeUnitId, _, _)> = Vec::with_capacity(self.units.len());
        for ImageUnit {
            mut code,
            constants,
        } in self.units
        {
            let table = install_constants(compiler, &atoms, constants)?;
            for insn in &mut code.code {
                if matches!(insn.op.format(), Format::Key | Format::AKey | Format::ABKey)
                    && let KeyKind::String(id) = insn.key().kind()
                {
                    insn.imm = PropertyKey::atom(atoms[id.0 as usize]).raw();
                }
            }
            code.name_atom = code.name_atom.map(|id| atoms[id.0 as usize]);
            let children: Vec<_> = code
                .children
                .iter()
                .map(|child| last - child.index())
                .collect();
            for child in &mut code.children {
                *child = linked[child.index() as usize].0;
            }
            // Validation ran against the wire tree. Relocation preserves the
            // operands and child relationships while replacing their IDs.
            let id = compiler.publish_validated(code);
            linked.push((id, table, children));
        }
        // The bytecode allocator takes the root first and explicit child
        // indexes. Reversing postorder supplies both without rebuilding the
        // tree by traversing the published code store.
        let templates: Vec<_> = linked
            .iter()
            .rev()
            .map(|(unit, constants, children)| BytecodeTemplate {
                unit: *unit,
                constants,
                children,
            })
            .collect();
        let prototype = compiler.intrinsic(Intrinsic::FunctionPrototype);
        compiler
            .heap_mut()
            .alloc_bytecode_function(Some(prototype), None, &templates)
    }
}

impl Runtime {
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
    reader: &mut Reader<'_>,
    strings: usize,
    filename: Option<Rc<str>>,
) -> Result<ImageUnit, ImageError> {
    let register_count = reader.u16()?;
    let parameter_count = reader.u16()?;
    let local_count = reader.u16()?;
    let flags = reader.u16()?;
    let this_slots = reader.u16()?;
    let spill_register = reader.u16()?;
    let spill_slots = reader.u16()?;
    let length = reader.u16()?;
    let ic_count = reader.u32()?;
    let name_atom = match reader.u32()? {
        NONE => None,
        index => Some(wire_atom(text_ref(index, strings)?)),
    };
    let name = Box::from(reader.text()?);
    let line = reader.u32()?;
    let column = reader.u32()?;
    let source = reader.blob()?;
    let source = (!source.is_empty()).then(|| Box::from(source));

    let code = reader.list(5, |reader| {
        let bytes = reader.take(5)?;
        let op = *Opcode::ALL
            .get(bytes[0] as usize)
            .ok_or(ImageError::Malformed)?;
        let imm = reader.var_u32_from(bytes[4])?;
        if matches!(op.format(), Format::Key | Format::AKey | Format::ABKey) {
            match PropertyKey::from_raw(imm).kind() {
                KeyKind::Index(_) => {}
                KeyKind::String(id) => {
                    text_ref(id.0, strings)?;
                }
                KeyKind::Symbol(_) => return Err(ImageError::Malformed),
            }
        }
        Ok(Insn::new(op, bytes[1], bytes[2], bytes[3], imm))
    })?;
    if ic_count as usize > code.len() || flags & !((flags::EVAL << 1) - 1) != 0 {
        return Err(ImageError::Malformed);
    }
    let handlers = reader.list(14, |reader| {
        Ok(Handler {
            start: reader.u32()?,
            end: reader.u32()?,
            target: reader.u32()?,
            register: reader.u16()?,
        })
    })?;
    let children = reader.list(4, |reader| Ok(CodeUnitId(reader.u32()?)))?;
    let captures = reader.list(3, |reader| {
        let tag = reader.u8()?;
        let index = reader.u16()?;
        match tag {
            0 if index == 0 => Ok(Capture::None),
            1 => Ok(Capture::Register(index)),
            2 => Ok(Capture::Outer(index)),
            3 => Ok(Capture::Slot(index)),
            _ => Err(ImageError::Malformed),
        }
    })?;
    let (mut pc, mut next_line, mut next_column) = (0u32, line, column);
    let positions = reader.list(3, |reader| {
        pc = pc
            .checked_add(reader.var_u32()?)
            .ok_or(ImageError::Malformed)?;
        next_line = next_line.wrapping_add(reader.signed_delta()?);
        next_column = next_column.wrapping_add(reader.signed_delta()?);
        Ok(SourcePosition {
            pc,
            line: next_line,
            column: next_column,
        })
    })?;
    let direct_eval_sites = reader.list(4, Reader::u32)?;
    if !direct_eval_sites.is_sorted()
        || direct_eval_sites
            .last()
            .is_some_and(|&pc| pc as usize >= code.len())
    {
        return Err(ImageError::Malformed);
    }
    let constants = reader.list(1, |reader| read_constant(reader, strings))?;
    let unit = CodeUnit {
        code: code.into(),
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
            positions: positions.into(),
            definition: (line != 0 || column != 0).then_some((line, column)),
        },
        name_atom,
        length,
        source,
        direct_eval_sites: direct_eval_sites.into(),
        atoms: Box::new([]),
    };
    Ok(ImageUnit {
        code: unit,
        constants,
    })
}

fn read_constant(reader: &mut Reader<'_>, strings: usize) -> Result<Constant, ImageError> {
    Ok(Constant::Scalar(match reader.u8()? {
        0 => Value::UNDEFINED,
        1 => Value::NULL,
        2 => Value::FALSE,
        3 => Value::TRUE,
        4 => Value::EMPTY,
        5 => Value::int32(reader.u32()? as i32),
        6 => Value::number(f64::from_bits(reader.u64()?)),
        7 => return Ok(Constant::String(text_ref(reader.u32()?, strings)?)),
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
