use super::*;

#[test]
fn runtime_preloads_quickjs_typeof_atoms_as_narrow_canonical_strings() {
    let runtime = Runtime::new();
    let initial_atom_count = runtime.test_atom_count();
    for spelling in crate::engine::vm::TYPEOF_STATIC_ATOMS {
        let key = runtime.intern_property_key(spelling).unwrap();
        let canonical = runtime.property_key_to_js_string(&key).unwrap();
        assert!(!canonical.is_wide());
        assert_eq!(runtime.test_atom_count(), initial_atom_count);
    }
}
