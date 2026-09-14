"""Lightweight semantic contract canaries; no Rust build or full boundary scan."""
import re
import unittest
from pathlib import Path
from .context import ScanContext
from .rules.publication_contracts import check_executable, check_verified, check_instruction

ROOT = Path(__file__).resolve().parents[3]
EXECUTABLE = "src/engine/code/executable.rs"
VERIFIED = "src/engine/code/verify/verified.rs"
INSTRUCTION = "src/engine/code/instruction.rs"
BYTECODE = "src/engine/code/bytecode.rs"


def errors(overrides=None):
    ctx = ScanContext(ROOT)
    ctx.raw_string_prefix = re.compile(r'(?:br|rb|cr|rc|r)(?P<hashes>#{0,255})"')
    ctx.read_source = lambda name: (overrides or {}).get(name, (ROOT / name).read_text())
    check_executable(ctx)
    check_verified(ctx)
    check_instruction(ctx)
    return ctx.errors


class PublicationContracts(unittest.TestCase):
    def test_current_refactor_passes(self):
        self.assertEqual(errors(), [])

    def test_ownership_and_canonical_fact_mutations_are_rejected(self):
        cached = "data: Rc<PublishedFunctionData>" in (ROOT / EXECUTABLE).read_text()
        owned = "fn snapshot_function_bytecode_owned" in (ROOT / EXECUTABLE).read_text()
        split = "fn stack_contract" in (ROOT / INSTRUCTION).read_text()
        data_type = "Rc<PublishedFunctionData>" if cached else "PublishedFunctionData"
        stack_before = "pub const fn stack_effect(&self) -> (usize, usize) {\n        " + (
            "self.nominal_stack_effect()" if split else "let effect = self.info().stack;\n        (effect.popped, effect.pushed)"
        )
        mutations = [
            (EXECUTABLE, "published-executable-owner", "if !function.belongs_to(self)" + (" {\n            return Err(RuntimeError::WrongRuntime(\"function bytecode\"));\n        }\n        let state" if owned else ""), "if false"),
            (EXECUTABLE, "published-executable-owner", f"data: {data_type},", f"pub(crate) data: {data_type},"),
            (EXECUTABLE, "published-executable-owner", "root: Some(function)," if owned else "root: Some(root),", "root: None,"),
            (EXECUTABLE, "published-executable-owner", "state.heap.context(bytecode.realm)?;", ""),
            (EXECUTABLE, "published-executable-owner", "function.bytecode_id())?;", "other.bytecode_id())?;"),
            (EXECUTABLE, "published-executable-owner", "code: bytecode.code.clone(),", "code: Rc::from([]),"),
            (EXECUTABLE, "published-executable-owner", "self.index == other.index && Rc::ptr_eq", "Rc::ptr_eq"),
            (EXECUTABLE, "published-executable-owner", "#[cfg(test)]\nimpl std::ops::DerefMut", "impl std::ops::DerefMut"),
            (EXECUTABLE, "published-executable-owner", "self.root.is_none(),", "true,"),
            (EXECUTABLE, "published-executable-owner", "pub(crate) constants: Rc<[BytecodeConstant]>,", "pub(crate) constants: std::cell::RefCell<Vec<BytecodeConstant>>,"),
            (VERIFIED, "published-function-verification", "verify_unlinked_ordinary_leaf(&function)?;", ""),
            (VERIFIED, "published-function-verification", "verify_unlinked_tree(&function)?;", "if false { verify_unlinked_tree(&function)?; }"),
            (VERIFIED, "published-function-verification", "VerifiedFunction(UnlinkedFunction)", "VerifiedFunction(pub(crate) UnlinkedFunction)"),
            (VERIFIED, "published-function-verification", "expected.arguments_forbidden,", "false,"),
            (INSTRUCTION, "published-instruction-contract", "let (popped, pushed) = self.nominal_stack_effect();", "let (popped, pushed) = (0, 0);"),
            (INSTRUCTION, "published-instruction-contract", "stack: self.stack_contract()," if split else "control: self.control_effect(),", "stack: wrong_stack()," if split else "control: ControlEffect::Next,"),
            (INSTRUCTION, "published-instruction-contract", "effects: self.potential_effects()," if split else "javascript_exception: self.javascript_exception_effect(),", "effects: wrong_effects()," if split else "javascript_exception: JsExceptionEffect::None,"),
            (INSTRUCTION, "published-instruction-contract", "may_call_js: self.may_call_js(),", "may_call_js: false,"),
            (INSTRUCTION, "published-instruction-contract", "state: self.stack_state_effect(),", "state: StackStateEffect::ConsumeSuperCall,"),
            (INSTRUCTION, "published-instruction-contract", "const fn static_name(self) -> Option<u32> {", "const fn static_name(self) -> Option<u32> { return None;"),
            (BYTECODE, "published-instruction-stack-adapter", stack_before, "pub const fn stack_effect(&self) -> (usize, usize) {\n        (0, 0)"),
        ]
        if owned:
            mutations.extend([
                (EXECUTABLE, "published-executable-owner", "self.snapshot_function_bytecode_owned(function.clone())", "self.unchecked_snapshot(function.clone())"),
                (EXECUTABLE, "published-executable-owner", "observes_arguments: bytecode.code.iter().any", "observes_arguments: false && bytecode.code.iter().any"),
            ])
        for path, rule, before, after in mutations:
            with self.subTest(rule=rule, mutation=before):
                source = (ROOT / path).read_text()
                self.assertEqual(source.count(before), 1, "canary anchor must identify exactly one real production site")
                result = errors({path: source.replace(before, after)})
                self.assertTrue(any(error.startswith(rule + ":") for error in result), result)


    def test_direct_s08_representation_and_mutations(self):
        from .rules.publication_contracts import (
            SNAPSHOT_FUNCTION, DIRECT_SNAPSHOT_FUNCTION, BORROWED_SNAPSHOT_FUNCTION, OWNED_SNAPSHOT_FUNCTION, function,
        )
        # Derive the old representation from the same production facts. This
        # fixture needs no ignored source export or git history to remain runnable.
        sources = {name: (ROOT / name).read_text() for name in (EXECUTABLE, VERIFIED, INSTRUCTION, BYTECODE)}
        source = sources[EXECUTABLE].split("#[cfg(test)]\nmod tests", 1)[0]
        source = source.replace(OWNED_SNAPSHOT_FUNCTION, "")
        source = source.replace(BORROWED_SNAPSHOT_FUNCTION, DIRECT_SNAPSHOT_FUNCTION)
        source = source.replace(SNAPSHOT_FUNCTION, DIRECT_SNAPSHOT_FUNCTION)
        source = source.replace("    pub(crate) observes_arguments: bool,\n", "")
        source = source.replace("                observes_arguments: true,\n", "")
        source = source.replace("data: Rc<PublishedFunctionData>,", "data: PublishedFunctionData,")
        source = source.replace("data: Rc::new(PublishedFunctionData {", "data: PublishedFunctionData {")
        source = source.replace("            }),\n        }\n    }", "            },\n        }\n    }")
        source = source.replace('Rc::get_mut(&mut self.data).expect("synthetic executable remains uniquely owned")', "&mut self.data")
        sources[EXECUTABLE] = source
        sources[VERIFIED] = re.sub(
            r'#\[cfg\(feature = "profiling"\)\]\s+let _phase_timer = crate::engine::api::profiling::PhaseTimer::start\(\s*crate::engine::api::profiling::CompilePhase::Verify,\s*\);',
            "", sources[VERIFIED],
        )
        ctx = ScanContext(ROOT)
        ctx.raw_string_prefix = re.compile(r'(?:br|rb|cr|rc|r)(?P<hashes>#{0,255})"')
        source = sources[INSTRUCTION]
        if "fn stack_contract" in source:
            for name in ("stack_contract", "potential_effects"):
                source = source.replace(function(ctx, source, name, "fixture"), "")
            source = source.replace(function(ctx, source, "info", "fixture"), """pub(crate) const fn info(&self) -> InstructionInfo {
                let (popped, pushed) = self.nominal_stack_effect();
                InstructionInfo {
                    stack: StackEffect { popped, pushed, state: self.stack_state_effect(), },
                    effects: PotentialEffects { javascript_exception: self.javascript_exception_effect(),
                        may_call_js: self.may_call_js(), may_allocate: self.may_allocate(), },
                    control: self.control_effect(), operands: self.operand_contract(),
                }
            }""")
            for name in ("nominal_stack_effect", "control_effect", "operand_contract"):
                source = source.replace(f"pub(crate) const fn {name}", f"const fn {name}")
        sources[INSTRUCTION] = source
        sources[BYTECODE] = sources[BYTECODE].replace(
            "pub const fn stack_effect(&self) -> (usize, usize) {\n        self.nominal_stack_effect()",
            "pub const fn stack_effect(&self) -> (usize, usize) {\n        let effect = self.info().stack;\n        (effect.popped, effect.pushed)",
        )
        self.assertEqual(errors(sources), [])
        mutations = [
            (EXECUTABLE, "published-executable-owner", "if !function.belongs_to(self)", "if false"),
            (EXECUTABLE, "published-executable-owner", "data: PublishedFunctionData,", "pub(crate) data: PublishedFunctionData,"),
            (EXECUTABLE, "published-executable-owner", "root: Some(root),", "root: None,"),
            (EXECUTABLE, "published-executable-owner", "state.heap.context(bytecode.realm)?;", ""),
            (EXECUTABLE, "published-executable-owner", "self.root.is_none(),", "true,"),
            (VERIFIED, "published-function-verification", "verify_unlinked_tree(&function)?;", "if false { verify_unlinked_tree(&function)?; }"),
            (INSTRUCTION, "published-instruction-contract", "let (popped, pushed) = self.nominal_stack_effect();", "let (popped, pushed) = (0, 0);"),
            (INSTRUCTION, "published-instruction-contract", "may_call_js: self.may_call_js(),", "may_call_js: false,"),
            (BYTECODE, "published-instruction-stack-adapter", "(effect.popped, effect.pushed)", "(0, 0)"),
        ]
        for path, rule, before, after in mutations:
            with self.subTest(stage="S08", rule=rule, mutation=before):
                self.assertEqual(sources[path].count(before), 1)
                mutated = dict(sources)
                mutated[path] = mutated[path].replace(before, after)
                self.assertTrue(any(error.startswith(rule + ":") for error in errors(mutated)))


if __name__ == "__main__":
    unittest.main()
