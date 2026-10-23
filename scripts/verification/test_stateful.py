import unittest

from phase1 import check_results
from stateful import fixed_array_bounds
from test_phase1 import successful_report
from verus import ROOT, mark_source


class StatefulTests(unittest.TestCase):
    def test_selects_fixed_loops_without_increasing_nested_loop_bound(self):
        source = 'for (class, list) in self.free.iter_mut().enumerate() {\nfor list in &mut self.free {'
        listing = '''Loop outer.1:
  file core/src/heap/values.rs line 1 column 9 function begin_sweep
Loop nested.0:
  file core/src/heap/values.rs line 3 column 13 function begin_sweep
Loop finish.1:
  file core/src/heap/values.rs line 2 column 9 function finish_sweep
Loop construct.0:
  function std::array::try_from_fn_erased<T>
Loop drop.0:
  function std::ptr::drop_glue::<[std::vec::Vec<u32>]>
'''
        self.assertEqual(fixed_array_bounds(listing, source),
                         ['construct.0:65', 'drop.0:65', 'finish.1:65', 'outer.1:65'])
        with self.assertRaises(ValueError):
            fixed_array_bounds('', source)
        with self.assertRaises(ValueError):
            fixed_array_bounds(listing, '')

    def test_custom_inventory_is_required(self):
        report = successful_report()
        with self.assertRaises(ValueError):
            check_results(report, {'different::proof'})

    def test_conditional_assertions_still_require_a_reachable_property(self):
        report = successful_report()
        checks = report["verification_results"]["results"][0]["checks"]
        checks.append({"status": "Unreachable", "description": "assertion failed: nonempty branch",
                       "location": {"file": "core/src/heap/arena_kani_tests.rs"}})
        check_results(report, strict_assertions=False)
        checks[0]["status"] = "Unreachable"
        with self.assertRaises(ValueError):
            check_results(report, strict_assertions=False)

    def test_source_projection_rejects_unreviewed_translation_changes(self):
        source = (ROOT / 'core/src/heap/arena.rs').read_text()
        template = (ROOT / 'scripts/verification/verus/arena_mark.rs.in').read_text()
        generated = mark_source(source, template)
        self.assertIn('self.check(index);', generated)
        self.assertIn('*bits |= mask;', generated)
        self.assertNotIn('/* MARK_BODY */', generated)
        for old, new in [('bits & mask != 0', 'bits & mask == 0'),
                         ('if CHECK_LIVE {', 'if !CHECK_LIVE {')]:
            with self.subTest(old=old), self.assertRaises(ValueError):
                mark_source(source.replace(old, new), template)
        # Changes to the verified body propagate to Verus, rather than using
        # a stale handwritten copy. A proof mutation check exercises this too.
        mutated = mark_source(source.replace('*bits |= mask;', '*bits &= !mask;'), template)
        self.assertIn('*bits &= !mask;', mutated)


if __name__ == '__main__':
    unittest.main()
