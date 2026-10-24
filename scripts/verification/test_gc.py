import importlib.util
from pathlib import Path
import unittest

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
        self.assertIn("decreases trace.remaining()", generated)

    def test_missing_or_unbalanced_kernel_is_rejected(self):
        for source in ("", "pub(crate) fn mark_word() { loop {"):
            with self.assertRaises(ValueError):
                gc.function(source, "mark_word")

    def test_unhandled_lexical_forms_are_rejected(self):
        for body in ('"}"', "/* } */", "// }\n0"):
            with self.assertRaises(ValueError):
                gc.function("pub(crate) fn mark_word() { " + body + " }", "mark_word")

    def test_trust_escape_is_rejected_but_comments_are_not(self):
        for escape in ("assume(false)", "admit()", "#[verifier::external_body]"):
            with self.assertRaises(ValueError):
                gc.reject_trust_escapes(escape)
        gc.reject_trust_escapes("// no assume or admit\nproof fn test() {}")


if __name__ == "__main__":
    unittest.main()
