//! The realm's handle and the types of its operations (`ops` defines the
//! operations themselves).

use super::RuntimeError;
#[cfg(feature = "test262-host")]
use super::property::PropertyKey;
use super::runtime::Runtime;
use super::value::ObjectRef;
use super::value::Value;

/// Options of `Context::eval_with_options`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalOptions {
    pub filename: String,
    /// The code is freed once nothing names it (a REPL's line), instead of
    /// living as long as the runtime.
    pub transient: bool,
}

impl EvalOptions {
    pub fn new(filename: impl Into<String>) -> Self {
        EvalOptions {
            filename: filename.into(),
            transient: false,
        }
    }

    /// These options with [`EvalOptions::transient`] set.
    #[must_use]
    pub fn transient(mut self) -> Self {
        self.transient = true;
        self
    }
}

impl Default for EvalOptions {
    fn default() -> Self {
        Self::new(crate::engine::compiler::DEFAULT_EVAL_FILENAME)
    }
}

/// A compiled script, run by `Context::execute`.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionBytecodeRef {
    function: ObjectRef,
    /// The front end's stack code, which the front end's tests inspect.
    #[cfg(test)]
    pub(crate) stack: StackCode,
}

/// A function of a compiled script's stack code: the script's unlinked tree
/// and the child-constant path from its root.
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct StackCode {
    pub(crate) root: std::rc::Rc<crate::engine::code::function::UnlinkedFunction>,
    pub(crate) path: Vec<usize>,
}

#[cfg(test)]
impl StackCode {
    pub(crate) fn function(&self) -> &crate::engine::code::function::UnlinkedFunction {
        self.path.iter().fold(&self.root, |function, index| {
            function.constants()[*index]
                .as_child()
                .expect("a child-function constant")
        })
    }
}

#[cfg(test)]
impl std::fmt::Debug for StackCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StackCode")
            .field("path", &self.path)
            .finish()
    }
}

#[cfg(test)]
impl PartialEq for StackCode {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.root, &other.root) && self.path == other.path
    }
}

/// A compiled module, run by `Context::execute_module`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleBytecodeRef {
    name: String,
}

impl ModuleBytecodeRef {
    /// The module's name (its filename).
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// qjs's helpers (`Context::install_qjs_helpers`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QjsHelpers {
    /// `load` and `read` (`qjs --std`).
    pub std: bool,
    /// `scriptArgs`, when set.
    pub script_args: Option<Vec<String>>,
}

/// The runtime's one realm.
#[derive(Clone)]
pub struct Context {
    pub(crate) runtime: Runtime,
}

impl FunctionBytecodeRef {
    pub(crate) fn new(
        function: ObjectRef,
        #[cfg(test)] source: &[u8],
        #[cfg(test)] filename: &str,
        #[cfg(test)] debug_info: crate::engine::code::debug::DebugInfoMode,
    ) -> Self {
        FunctionBytecodeRef {
            function,
            #[cfg(test)]
            stack: StackCode {
                root: std::rc::Rc::new(
                    crate::engine::compiler::compile_unlinked_script_bytes_with_filename(
                        source, filename, debug_info,
                    )
                    .expect("the script compiled once"),
                ),
                path: Vec::new(),
            },
        }
    }

    pub(crate) fn function(&self) -> &ObjectRef {
        &self.function
    }
}

impl ModuleBytecodeRef {
    pub(crate) fn new(name: &str) -> Self {
        ModuleBytecodeRef {
            name: name.to_owned(),
        }
    }
}

/// The handle of the runtime's one realm. Its operations are listed with
/// the [`Js`](super::Js) trait, which a host function's
/// [`Scope`](super::Scope) shares; each is an inherent method and one
/// engine entry.
impl Context {
    /// The runtime's realm (a second call returns the same one).
    pub fn new(runtime: &Runtime) -> Self {
        runtime.context()
    }

    pub fn try_clone(&self) -> Result<Self, RuntimeError> {
        Ok(self.clone())
    }

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Makes `exports` the module `name`: an `import` of the name sees each
    /// value under its name (`default` is the default export), as the
    /// module a [`ModuleLoader`](super::ModuleLoader) returns as
    /// `ModuleSource::Synthetic`. A second definition of the name replaces
    /// the first for later imports.
    pub fn define_module(
        &self,
        name: &str,
        exports: Vec<(String, Value)>,
    ) -> Result<(), RuntimeError> {
        let mut entry = self.runtime.enter()?;
        {
            let view = entry.view();
            for (_, value) in &exports {
                if let Value::Object(ObjectRef { slot })
                | Value::Symbol(super::value::SymbolRef { slot }) = value
                {
                    view.own(slot)?;
                }
            }
        }
        entry.inner.core.forget_module(name);
        self.runtime.shared.modules.borrow_mut().insert(
            name.to_owned(),
            super::runtime::Preloaded::Synthetic(exports),
        );
        Ok(())
    }

    /// Defines the Test262 host object `$262`; returns it.
    #[cfg(feature = "test262-host")]
    pub fn install_test262_host(&mut self) -> Result<ObjectRef, RuntimeError> {
        {
            let mut entry = self.runtime.enter()?;
            entry.inner.core.install_test262_host()?;
        }
        let global = self.global_object()?;
        let key = PropertyKey::from_js_string(crate::engine::value::JsString::from_static("$262"));
        match self.get_property(&global, &key)? {
            Value::Object(host) => Ok(host),
            _ => Err(RuntimeError::Invariant("$262 is an object")),
        }
    }
}
