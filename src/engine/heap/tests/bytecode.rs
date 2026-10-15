use super::*;

#[test]
fn bytecode_debug_filename_atom_transfers_to_the_node() {
    let mut heap = Heap::new();
    let shape = empty_shape(&mut heap);
    let prototype = heap
        .allocate_object(ObjectData::ordinary(shape, Vec::new()))
        .unwrap();
    let realm = heap
        .allocate_context(ContextData::new(
            prototype, prototype, prototype, prototype, prototype, prototype, prototype, prototype,
        ))
        .unwrap();
    let code: Rc<[Instruction]> = Rc::from([Instruction::Undefined, Instruction::Return]);
    let filename = Atom::from_raw(19);
    let mut owned = bytecode(&code, realm, Vec::new(), vec![filename]);
    owned.metadata.max_stack = 1;
    owned.debug = Some(FunctionDebugInfo {
        filename,
        pc2line: Some(Pc2LineTable::new(LineColumn::new(0, 0), Vec::new())),
        source: None,
    });
    let bytecode = heap.allocate_function_bytecode(owned).unwrap();
    assert_eq!(
        heap.release_function_bytecode(bytecode).unwrap().atoms,
        vec![AtomIdx::from_raw(filename.raw())]
    );
}

#[test]
fn bytecode_allocation_accepts_typed_class_heritage() {
    let mut heap = Heap::new();
    let shape = empty_shape(&mut heap);
    let prototype = leaf(&mut heap, shape);
    let context = heap
        .allocate_context(ContextData::new(
            prototype, prototype, prototype, prototype, prototype, prototype, prototype, prototype,
        ))
        .unwrap();
    heap.release_object(prototype).unwrap();

    let code: Rc<[Instruction]> = Rc::from([
        Instruction::Undefined,
        Instruction::PushI32(7),
        Instruction::DefineClass {
            name: 0,
            has_heritage: true,
        },
        Instruction::Drop,
        Instruction::Return,
    ]);
    let derived = heap
        .allocate_string(JsString::from_static("Derived"))
        .unwrap();
    let mut candidate = bytecode(
        &code,
        context,
        vec![BytecodeConstant::Value(RawValue::String(derived))],
        Vec::new(),
    );
    let mut atoms = crate::engine::atom::AtomTable::new();
    let name = atoms.intern("Derived").unwrap();
    candidate.property_key_atoms = Some(Rc::from([name]));
    candidate.auxiliary_atoms = Box::new([name]);
    candidate.metadata.max_stack = 2;
    let function = heap.allocate_function_bytecode(candidate).unwrap();
    heap.release_function_bytecode(function).unwrap();
    heap.release_string(derived).unwrap();

    heap.release_context(context).unwrap();
    heap.release_shape(shape).unwrap();
    assert_eq!(heap.counts().live, 0);
}
