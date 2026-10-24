import importlib.util
from pathlib import Path
import unittest

from source import item, masked, method

SPEC = importlib.util.spec_from_file_location("gc_proofs", Path(__file__).with_name("gc.py"))
gc = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gc)


class ProofSourceTests(unittest.TestCase):
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
