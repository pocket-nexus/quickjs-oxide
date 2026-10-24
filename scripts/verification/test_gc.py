import importlib.util
from pathlib import Path
import unittest

from source import item, masked, method
from records import record, tuple_enum

SPEC = importlib.util.spec_from_file_location("gc_proofs", Path(__file__).with_name("gc.py"))
gc = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gc)


class ProofSourceTests(unittest.TestCase):
    def test_enum_projection_retains_variants_and_rejects_unsupported_forms(self):
        original = '''pub enum Data {
            #[default]
            None,
            Table(Box<Table>),
            Bytes(Vec<u8>),
            Host(Rc<dyn Any>),
        }'''
        generated, parameters, variants = tuple_enum(original, "Data", "D", {"Table": ("Box<Table>", "Box<Table<C>>")})
        self.assertEqual(parameters, ["DBytes", "DHost"])
        self.assertEqual(variants, ["Bytes", "Host", "None", "Table"])
        self.assertIn("Table(Box<Table<C>>)", generated)
        self.assertIn("Bytes(DBytes)", generated)
        self.assertIn("Host(DHost)", generated)
        self.assertNotIn("#[default]", generated)
        for replacement in ("Table(Vec<Table>),", "Table,", "Table(u32, u32),",
                            "Table { field: u32 },", "Table = 1,", "Table(Box<\nTable>),",
                            "Table(Box<Table>), Host(u32),", '#[cfg(feature = "x")]\nTable(Box<Table>),'):
            with self.assertRaises(ValueError):
                tuple_enum(original.replace("Table(Box<Table>),", replacement), "Data", "D", {"Table": ("Box<Table>", "Box<Table<C>>")})

    def test_parametric_records_preserve_every_field_and_reject_drift(self):
        original = """pub struct State {
            known: Vec<u32>,
            ignored: Option<(usize, Vec<u8>)>,
            other: HostState,
            #[cfg(feature = "gc-stress")]
            stress: bool,
        }"""
        generated, parameters, fields = record(original, "State", "S", {"known": ("Vec<u32>", "Vec<u32>")})
        self.assertEqual(parameters, ["SIgnored", "SOther"])
        self.assertEqual(fields, ["ignored", "known", "other", "stress"])
        self.assertIn('known: Vec<u32>', generated)
        self.assertIn('ignored: SIgnored', generated)
        self.assertIn('#[cfg(feature = "gc-stress")]', generated)
        self.assertIn('stress: bool', generated)
        for broken in (original.replace('Vec<u32>', 'Vec<u64>'),
                       original.replace('other: HostState,', 'other: HostState, hidden: u64,'),
                       original.replace('other: HostState,', 'other: Option<\nHostState>,')):
            with self.assertRaises(ValueError):
                record(broken, "State", "S", {"known": ("Vec<u32>", "Vec<u32>")})

    def test_kernel_body_is_copied_verbatim(self):
        source = (gc.ROOT / "core/src/heap/gc_kernel.rs").read_text()
        generated = gc.kernel_proof(source)
        for name in ("mark_word", "dead_word", "ephemeron_action"):
            _, body = gc.function(source, name)
            self.assertIn("let result = " + body + ";", generated)

    def test_driver_only_adds_ghost_annotations(self):
        source = (gc.ROOT / "core/src/heap/gc_kernel.rs").read_text()
        generated = gc.driver_proof(source)
        self.assertIn("trace.scan()", generated)
        self.assertIn("trace.drain();", generated)
        self.assertIn("trace.clear();", generated)
        self.assertIn("decreases remaining(trace)", generated)

    def test_missing_or_unbalanced_kernel_is_rejected(self):
        for source in ("", "pub(crate) fn mark_word() { loop {"):
            with self.assertRaises(ValueError):
                gc.function(source, "mark_word")

    def test_unhandled_lexical_forms_are_rejected(self):
        for body in ('"}"', "/* } */", "// }\n0"):
            with self.assertRaises(ValueError):
                gc.function("pub(crate) fn mark_word() { " + body + " }", "mark_word")

    def test_trust_escape_is_rejected_but_comments_are_not(self):
        for escape in ("assume(false)", "admit()", "#[verifier::external_body]",
                       "assume_specification[ Heap::drain ]() ensures false;",
                       "#[verifier::external]", "uninterp spec fn valid() -> bool;"):
            with self.assertRaises(ValueError):
                gc.reject_trust_escapes(escape)
        gc.reject_trust_escapes("// no assume or admit\nproof fn test() {}")

    def test_scoped_source_extraction_skips_nested_comments_and_strings(self):
        source = '''impl Roots {
            /* fake fn mark() { /* nested } */ } */
            pub fn mark(&self) -> usize { log("} escaped \\\" {"); 7 }
        }
        impl Other { fn mark() { 0 } }
        '''
        scope = item(source, r"impl Roots\b")
        selected = item(scope, r"pub fn mark\b")
        self.assertTrue(selected.endswith('7 }'))
        self.assertIn('log("}', selected)
        self.assertNotIn("Other", scope)
        with self.assertRaises(ValueError):
            item(source, r"(?:pub )?fn mark\b")

    def test_array_type_semicolon_does_not_terminate_item(self):
        self.assertEqual(item("const TABLE: [u32; 2] = [1, 2];", r"const TABLE\b"),
                         "const TABLE: [u32; 2] = [1, 2];")

    def test_byte_characters_cannot_change_item_boundaries(self):
        for literal in ("b'{'", "b'}'", "b';'", "b'\\x7b'", "b'\\n'", "b'\\''", "b'\\\\'"):
            source = "fn read<'a>(value: &'a u8) { let byte = " + literal + "; }"
            self.assertEqual(item(source, r"fn read\b"), source)
            self.assertEqual(len(masked(source)), len(source))
            self.assertNotIn(literal, masked(source))
        for literal in ("b'", "b'xx'", "b'\\q'", "b'\\x0'", "b'\u00e9'"):
            with self.assertRaises(ValueError):
                masked(literal)

    def test_bad_or_unsupported_lexical_input_is_rejected(self):
        for source in ('r#"text"#', "'x'", '"unterminated', '/* missing end'):
            with self.assertRaises(ValueError):
                masked(source)
        with self.assertRaises(ValueError):
            item("impl Roots { fn mark() {", r"impl Roots\b")

    def test_method_keeps_operational_body(self):
        original = 'pub fn read(&self, i: usize) -> u64 { self.words[i] }'
        generated = method(original, "read", "requires i < self.words@.len(),", "result")
        self.assertIn("{ self.words[i] }", generated)
        self.assertIn("-> (result: u64)", generated)

    def test_ghost_annotations_require_unique_points(self):
        for source in ("fn go() {}", "fn go() { step(); step(); }"):
            with self.assertRaises(ValueError):
                method(source, "go", "", before={"step();": "assert(true);"})
            with self.assertRaises(ValueError):
                method(source, "go", "", bindings={"step();": {"saved": "0"}})
        with self.assertRaises(ValueError):
            method("fn go() { let saved = 0; step(); }", "go", "",
                   bindings={"step();": {"saved": "1"}})

    def test_panic_projection_preserves_known_guard(self):
        original = 'fn check(&self) { assert!(self.live(), "{}", type_name::<T>()); }'
        generated = method(original, "check", "requires self.valid(),", panic_condition="self.live()")
        self.assertIn("assert!(self.live());", generated)
        self.assertNotIn("type_name", generated)
        for changed in (original.replace("self.live()", "false"),
                        original.replace("; }", "; assert!(true); }")):
            with self.assertRaises(ValueError):
                method(changed, "check", "", panic_condition="self.live()")

    def test_source_proof_generators_are_complete_and_have_no_trust_escapes(self):
        heap = (gc.ROOT / "core/src/heap/gc.rs").read_text()
        value = (gc.ROOT / "core/src/value.rs").read_text()
        arena = (gc.ROOT / "core/src/heap/arena.rs").read_text()
        for code in (
            gc.root_proofs.proof(heap, value, (gc.PROOFS / "roots.rs.in").read_text()),
            gc.value_proofs.proof(value, (gc.PROOFS / "value.rs.in").read_text(), heap),
            gc.arena_proofs.proof(arena, (gc.PROOFS / "arena.rs.in").read_text()),
            gc.waiter_proofs.proof((gc.ROOT / "core/src/heap/gc_waiters.rs").read_text(),
                                  value, (gc.PROOFS / "waiters.rs.in").read_text()),
        ):
            self.assertNotIn("// METHOD", code)
            gc.reject_trust_escapes(code)
        scan = gc.scan_proofs.proof((gc.ROOT / "core/src/heap/gc_scan.rs").read_text(),
            (gc.PROOFS / "scan.rs.in").read_text(),
            gc.value_proofs.proof(value, (gc.PROOFS / "value.rs.in").read_text(), heap),
            gc.waiter_proofs.proof((gc.ROOT / "core/src/heap/gc_waiters.rs").read_text(),
                                  value, (gc.PROOFS / "waiters.rs.in").read_text()),
            gc.kernel_proof((gc.ROOT / "core/src/heap/gc_kernel.rs").read_text()),
            gc.root_proofs.proof(heap, value, (gc.PROOFS / "roots.rs.in").read_text()))
        self.assertNotIn("// SCAN_ENTRIES", scan)
        self.assertNotIn("// METHOD", scan)
        gc.reject_trust_escapes(scan)
        weakpass = gc.weakpass_proofs.proof((gc.ROOT / "core/src/heap/mod.rs").read_text(),
            heap, (gc.ROOT / "core/src/heap/object.rs").read_text(),
            gc.arena_proofs.proof(arena, (gc.PROOFS / "arena.rs.in").read_text()), scan)
        gc.reject_trust_escapes(weakpass)
        symbol = gc.symbol_proofs.proof((gc.ROOT / "core/src/heap/mod.rs").read_text(),
            heap, (gc.ROOT / "core/src/heap/object.rs").read_text(),
            (gc.ROOT / "core/src/heap/symbol.rs").read_text(),
            gc.arena_proofs.proof(arena, (gc.PROOFS / "arena.rs.in").read_text()), scan)
        gc.reject_trust_escapes(symbol)
        reuse = gc.reuse_proofs.proof(arena,
            gc.arena_proofs.proof(arena, (gc.PROOFS / "arena.rs.in").read_text()))
        gc.reject_trust_escapes(reuse)
        root_trace = gc.root_trace_proofs.proof(heap,
            gc.root_proofs.proof(heap, value, (gc.PROOFS / "roots.rs.in").read_text()),
            gc.value_proofs.proof(value, (gc.PROOFS / "value.rs.in").read_text(), heap))
        gc.reject_trust_escapes(root_trace)
        jobs = gc.job_trace_proofs.proof((gc.ROOT / "core/src/vm/jobs.rs").read_text(),
            gc.value_proofs.proof(value, (gc.PROOFS / "value.rs.in").read_text(), heap))
        gc.reject_trust_escapes(jobs)
        vm_roots = gc.vm_root_proofs.proof((gc.ROOT / "core/src/heap/mod.rs").read_text(), heap,
            (gc.ROOT / "core/src/vm/mod.rs").read_text(),
            gc.value_proofs.proof(value, (gc.PROOFS / "value.rs.in").read_text(), heap))
        gc.reject_trust_escapes(vm_roots)
        vm_resume = gc.vm_resume_proofs.proof((gc.ROOT / "core/src/heap/mod.rs").read_text(),
            (gc.ROOT / "core/src/vm/mod.rs").read_text(), (gc.ROOT / "core/src/vm/interp.rs").read_text(),
            value, vm_roots)
        gc.reject_trust_escapes(vm_resume)
        cleanup = gc.weak_cleanup_proofs.proof((gc.ROOT / "core/src/heap/mod.rs").read_text(),
            heap, (gc.ROOT / "core/src/heap/object.rs").read_text(), value,
            gc.arena_proofs.proof(arena, (gc.PROOFS / "arena.rs.in").read_text()), scan)
        gc.reject_trust_escapes(cleanup)
        clear_scope = item(item(heap, r"impl Heap\b"), r"fn clear_weak\b")
        arm = item(clear_scope, r"InternalTag::WeakRef\s*=>")
        self.assertIn(arm[arm.index("{"):-1], cleanup)
        finalization = gc.finalization_proofs.proof((gc.ROOT / "core/src/heap/gc_scan.rs").read_text(),
            value, jobs, scan, (gc.PROOFS / "finalization.rs.in").read_text())
        gc.reject_trust_escapes(finalization)
        heap_source = (gc.ROOT / "core/src/heap/mod.rs").read_text()
        object_source = (gc.ROOT / "core/src/heap/object.rs").read_text()
        arena_code = gc.arena_proofs.proof(arena, (gc.PROOFS / "arena.rs.in").read_text())
        finalization_heap = gc.finalization_proofs.heap_proof(heap_source, heap, object_source,
            arena_code, finalization)
        gc.reject_trust_escapes(finalization_heap)
        self.assertIn("jobs: std::collections::VecDeque<Job>", finalization_heap)
        with self.assertRaisesRegex(ValueError, "jobs"):
            gc.finalization_proofs.heap_proof(heap_source.replace(
                "std::collections::VecDeque<crate::vm::jobs::Job>", "Vec<crate::vm::jobs::Job>"),
                heap, object_source, arena_code, finalization)

    def test_cleanup_arm_selection_rejects_missing_or_duplicate_guards(self):
        heap = (gc.ROOT / "core/src/heap/gc.rs").read_text()
        guard = "InternalTag::WeakRef => {"
        for changed in (heap.replace(guard, "InternalTag::WeakSet => {"),
                        heap.replace(guard, guard + "}\n" + guard)):
            with self.assertRaises(ValueError):
                gc.weak_cleanup_proofs.proof("", changed, "", "", "", "")

    def test_finalization_arm_selection_rejects_missing_or_duplicate_guards(self):
        heap = (gc.ROOT / "core/src/heap/gc.rs").read_text()
        scope = item(item(heap, r"impl Heap\b"), r"fn clear_weak\b")
        guard = "InternalTag::FinalizationRegistry => {"
        for changed in (scope.replace(guard, "InternalTag::WeakSet => {"),
                        scope.replace(guard, guard + "}\n" + guard)):
            with self.assertRaises(ValueError):
                gc.finalization_proofs.heap_proof("", "impl Heap {" + changed + "}", "", "", "")

    def test_table_index_projection_keeps_cursors_and_rejects_type_drift(self):
        source = (gc.ROOT / "core/src/heap/table.rs").read_text()
        generated = gc.table_index_proofs.proof(source)
        gc.reject_trust_escapes(generated)
        self.assertIn("cursors: Vec<C>", generated)
        self.assertIn("self.index[bucket] = entry as u32 + 1;", generated)
        with self.assertRaises(ValueError):
            gc.table_index_proofs.proof(source.replace("index: Vec<u32>", "index: Vec<u64>"))

    def test_table_deletion_projection_has_no_trust_escapes(self):
        heap = (gc.ROOT / "core/src/heap/gc.rs").read_text()
        value = (gc.ROOT / "core/src/value.rs").read_text()
        table = (gc.ROOT / "core/src/heap/table.rs").read_text()
        generated = gc.table_delete_proofs.proof(table,
            (gc.ROOT / "core/src/heap/object.rs").read_text(),
            gc.value_proofs.proof(value, (gc.PROOFS / "value.rs.in").read_text(), heap),
            gc.root_proofs.proof(heap, value, (gc.PROOFS / "roots.rs.in").read_text()),
            gc.table_index_proofs.proof(table))
        gc.reject_trust_escapes(generated)
        self.assertIn("table.index[bucket] = DELETED;", generated)
        self.assertIn("slots[position] = Value::EMPTY;", generated)

    def test_root_fields_cannot_silently_escape_tracing_policy(self):
        source = (gc.ROOT / "core/src/heap/gc.rs").read_text()
        changed = source.replace("pub fixed: Vec<Value>,", "pub fixed: Vec<Value>,\n    extra: Vec<Value>,")
        with self.assertRaisesRegex(ValueError, "Roots fields changed"):
            gc.root_trace_proofs.proof(changed, "", "")

    def test_job_fields_cannot_silently_escape_tracing_policy(self):
        source = (gc.ROOT / "core/src/vm/jobs.rs").read_text()
        changed = source.replace("rejected: bool,", "rejected: bool,\n        extra: Value,", 1)
        with self.assertRaisesRegex(ValueError, "Job fields changed"):
            gc.job_trace_proofs.proof(changed, "")

    def test_completeness_does_not_count_conditional_or_tested_code(self):
        obligations = [
            {"id": "proved", "status": "proved-source"},
            {"id": "conditional", "status": "proved-source-with-contracts"},
            {"id": "tested", "status": "runtime-checked"},
            {"id": "caller", "status": "caller-obligation"},
            {"id": "unknown", "status": "future-status"},
            {"id": "hidden", "status": "assumption"},
            {"id": "GC-RESOURCES", "status": "assumption"},
        ]
        self.assertEqual(gc.open_obligations(obligations),
                         ["conditional", "tested", "caller", "unknown", "hidden"])


if __name__ == "__main__":
    unittest.main()
