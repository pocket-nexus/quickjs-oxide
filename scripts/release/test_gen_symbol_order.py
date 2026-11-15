"""Version-only relinks retain symbol identities and equivalent folded bodies."""
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "gen_symbol_order", Path(__file__).with_name("gen-symbol-order.py"))
order = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = order
spec.loader.exec_module(order)
Symbol = order.Symbol


def unexpected_code(symbol):
    raise AssertionError(f"known symbol should not require disassembly: {symbol}")


class PreserveLayoutTests(unittest.TestCase):
    def test_nm_reader_includes_sizeless_crt_symbols(self):
        raw = "00000020 t frame_dummy\n00000040 00000010 t mangled\n"
        demangled = "00000020 t frame_dummy\n00000040 00000010 t demo::function\n"
        with patch.object(order.subprocess, "check_output", side_effect=[raw, demangled]):
            self.assertEqual(order.function_symbols("qjs"), [
                Symbol(32, 0, "frame_dummy", "frame_dummy"),
                Symbol(64, 16, "mangled", "demo::function"),
            ])

    def test_crate_version_hash_changes_keep_the_previous_physical_order(self):
        old = [Symbol(100, 20, "_RNvCsOLD_4demo4fast", "demo::fast"),
               Symbol(200, 40, "_RNvCsOLD_4demo4slow", "demo::slow")]
        new = [Symbol(900, 20, "_RNvCsNEW_4demo4fast", "demo::fast"),
               Symbol(500, 40, "_RNvCsNEW_4demo4slow", "demo::slow")]
        self.assertEqual(order.preserve_layout(old, new, unexpected_code, unexpected_code),
                         [new[0].name, new[1].name])

    def test_identity_matches_reserve_their_candidate_before_ambiguous_labels(self):
        old = [Symbol(100, 20, "unmatched", "shared"),
               Symbol(200, 20, "known", "shared")]
        new = [Symbol(300, 20, "known", "shared"),
               Symbol(400, 20, "other", "shared")]
        self.assertEqual(order.preserve_layout(old, new, unexpected_code, unexpected_code),
                         ["other", "known"])

    def test_equivalent_folded_body_can_change_its_representative_name(self):
        old = [Symbol(100, 20, "old_type", "old::type")]
        new = [Symbol(300, 20, "new_type", "new::type")]
        code = lambda symbol: ("push rbx", "call allocator", "ret")
        self.assertEqual(order.preserve_layout(old, new, code, code), ["new_type"])

    def test_changed_size_and_different_body_are_not_assigned_a_previous_slot(self):
        old = [Symbol(100, 20, "known", "known"), Symbol(200, 30, "old", "old")]
        new = [Symbol(300, 21, "known", "known"), Symbol(400, 30, "new", "new")]
        self.assertEqual(order.preserve_layout(
            old, new, lambda symbol: ("add $1",), lambda symbol: ("add $2",)), [])

    def test_aliases_at_one_address_do_not_duplicate_a_function_chunk(self):
        old = [Symbol(100, 20, "one", "one"), Symbol(100, 20, "alias", "alias")]
        new = [Symbol(300, 20, "one", "one"), Symbol(300, 20, "alias", "alias")]
        self.assertEqual(order.preserve_layout(old, new, unexpected_code, unexpected_code), ["one"])

    def test_sizeless_crt_function_keeps_its_text_chunk_in_place(self):
        old = [Symbol(100, 20, "before", "before"),
               Symbol(200, 0, "frame_dummy", "frame_dummy"),
               Symbol(300, 20, "after", "after")]
        new = [Symbol(900, 20, "before", "before"),
               Symbol(800, 20, "after", "after"),
               Symbol(1000, 0, "frame_dummy", "frame_dummy")]
        self.assertEqual(order.preserve_layout(old, new, unexpected_code, unexpected_code),
                         ["before", "frame_dummy", "after"])

    def test_profiled_interpreter_is_first_and_other_functions_keep_their_order(self):
        symbols = [Symbol(100, 20, "dispatch", order.INTERPRETER)]
        self.assertEqual(order.interpreter_first(["first", "dispatch", "last"], symbols),
                         ["dispatch", "first", "last"])

    def test_missing_or_ambiguous_interpreter_is_not_silently_replaced(self):
        with self.assertRaises(ValueError):
            order.interpreter_first(["other"], [])
        with self.assertRaises(ValueError):
            order.interpreter_first(["one", "two"], [
                Symbol(100, 20, "one", order.INTERPRETER),
                Symbol(200, 20, "two", order.INTERPRETER),
            ])


if __name__ == "__main__":
    unittest.main()
