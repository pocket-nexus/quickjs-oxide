//! Evaluation helpers shared by the front end's unit tests.

use crate::engine::api::core::{
    Context, FunctionBytecodeRef, ObjectRef, Runtime, RuntimeError, Value,
};
use crate::engine::code::bytecode::Instruction;
use crate::engine::value::JsString;
use crate::source::LineColumn;

/// A fresh runtime and its context.
pub(crate) fn test_context() -> (Runtime, Context) {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    (runtime, context)
}

/// Evaluate a self-checking script that returns `true` on success.
#[track_caller]
pub(crate) fn assert_script_true(context: &mut Context, source: &str) {
    assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
}

/// The front end's tests read a compiled script's stack code, which the core
/// does not keep, from the unlinked tree [`Context::compile`] retains in tests.
impl Runtime {
    pub(crate) fn test_child_function_bytecode(
        &self,
        function: &FunctionBytecodeRef,
        constant_index: usize,
    ) -> Result<FunctionBytecodeRef, RuntimeError> {
        let mut child = function.clone();
        if child.stack.function().constants()[constant_index]
            .as_child()
            .is_none()
        {
            return Err(RuntimeError::Invariant(
                "requested child constant is a value",
            ));
        }
        child.stack.path.push(constant_index);
        Ok(child)
    }

    pub(crate) fn test_function_code(
        &self,
        function: &FunctionBytecodeRef,
    ) -> Result<Vec<Instruction>, RuntimeError> {
        Ok(function.stack.function().code().to_vec())
    }

    pub(crate) fn test_function_name(
        &self,
        function: &FunctionBytecodeRef,
    ) -> Result<Option<JsString>, RuntimeError> {
        Ok(function.stack.function().func_name().cloned())
    }

    pub(crate) fn test_function_debug_location(
        &self,
        function: &FunctionBytecodeRef,
        pc: Option<usize>,
    ) -> Result<Option<(JsString, LineColumn)>, RuntimeError> {
        let Some(debug) = function.stack.function().debug() else {
            return Ok(None);
        };
        Ok(debug.pc2line.as_ref().map(|table| {
            (
                debug.filename.clone(),
                table.lookup(pc.and_then(|pc| u32::try_from(pc).ok())),
            )
        }))
    }

    pub(crate) fn test_function_debug_source(
        &self,
        function: &FunctionBytecodeRef,
    ) -> Result<Option<Vec<u8>>, RuntimeError> {
        Ok(function
            .stack
            .function()
            .debug()
            .and_then(|debug| debug.source.as_deref())
            .map(<[u8]>::to_vec))
    }

    pub(crate) fn prevent_extensions(&self, object: &ObjectRef) -> Result<(), RuntimeError> {
        let context = self.context();
        let Value::Object(prevent) = context.eval("Object.preventExtensions")? else {
            unreachable!("Object.preventExtensions is a function");
        };
        let prevent = self.as_callable(&prevent)?.expect("callable");
        context.call(&prevent, Value::Undefined, &[Value::Object(object.clone())])?;
        Ok(())
    }
}
