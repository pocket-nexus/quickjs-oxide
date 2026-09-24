pub(crate) use super::executable::{
    OrdinaryAuthentication, PublishedEvalEnvironment, PublishedFunctionData,
    PublishedFunctionSnapshot,
};
#[cfg(feature = "test262-host")]
use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::code::bytecode_publish;

use crate::engine::code::function::metadata::{
    ClosureVariable, ClosureVariableName, EvalEnvironment, FunctionMetadata,
    ParameterEnvironmentLayout, VariableDefinition,
};
use crate::engine::code::function::{
    UnlinkedConstant, UnlinkedFunction, UnlinkedFunctionDebug, UnlinkedVariableDefinition,
};
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::heap::ownership::ConvertedValue;
use crate::engine::heap::{
    BytecodeConstant, ContextId, FunctionBytecodeData, FunctionDebugInfo, PublishedPrivateBinding,
    PublishedPrivateBindings, RawValue,
};
use crate::engine::object::ObjectRef;
use crate::engine::value::{JsString, Value};
#[cfg(test)]
use crate::source::LineColumn;

impl Runtime {
    /// Publish an immutable bytecode GC node in `realm` from an unlinked
    /// compiler draft.
    pub(crate) fn publish_unlinked_function(
        &self,
        realm: ContextId,
        function: UnlinkedFunction,
    ) -> Result<FunctionBytecodeRef, RuntimeError> {
        #[cfg(feature = "profiling")]
        let _phase_timer = crate::engine::api::profiling::PhaseTimer::start(
            crate::engine::api::profiling::CompilePhase::Publish,
        );
        let _operation = self.operation();

        let mut frames = vec![PublishFrame::new(function)];
        self.ensure_dynamic_import_bytecode_allowed(&frames[0].code)?;

        loop {
            let next = frames
                .last_mut()
                .ok_or(RuntimeError::Invariant(
                    "unlinked function publication lost its root frame",
                ))?
                .remaining
                .next();
            if let Some(constant) = next {
                let constant = match constant.into_template_object() {
                    Ok((cooked, raw)) => {
                        let frame = frames
                            .last_mut()
                            .expect("publication frame remains present");
                        let template = self.instantiate_template_object(realm, cooked, raw)?;
                        frame
                            .linked_constants
                            .push(BytecodeConstant::Value(RawValue::Object(
                                template.object_id(),
                            )));
                        frame.materialized_constant_roots.push(template);
                        continue;
                    }
                    Err(constant) => constant,
                };
                let constant = match constant.into_regexp() {
                    Ok((pattern, program)) => {
                        frames
                            .last_mut()
                            .expect("publication frame remains present")
                            .linked_constants
                            .push(BytecodeConstant::RegExp { pattern, program });
                        continue;
                    }
                    Err(constant) => constant,
                };
                let (primitive, atom_string, child) = constant.into_parts();
                match (primitive, atom_string, child) {
                    (Some(crate::engine::value::PrimitiveValue::String(value)), true, None) => {
                        // Keep the compiler-owned payload outside the arena
                        // until atom canonicalization selects its final form.
                        // This unpublished slot is filled before any constant
                        // consumers or bytecode publication can observe it.
                        let frame = frames
                            .last_mut()
                            .expect("publication frame remains present");
                        frame
                            .atom_string_constants
                            .push((frame.linked_constants.len(), value));
                        frame
                            .linked_constants
                            .push(BytecodeConstant::Value(RawValue::Undefined));
                    }
                    (Some(value), false, None) => {
                        let value: Value = value.into();
                        let converted = self.raw_property_value(&value)?;
                        let frame = frames
                            .last_mut()
                            .expect("publication frame remains present");
                        frame
                            .linked_constants
                            .push(BytecodeConstant::Value(converted.raw()));
                        frame.converted_constants.push(converted);
                    }
                    (None, false, Some(child)) => {
                        let frame = PublishFrame::new(child);
                        self.ensure_dynamic_import_bytecode_allowed(&frame.code)?;
                        frames.push(frame);
                    }
                    (None, _, None)
                    | (Some(_), true, None)
                    | (Some(_), _, Some(_))
                    | (None, true, Some(_)) => {
                        return Err(RuntimeError::Invariant(
                            "unlinked constant did not contain exactly one payload",
                        ));
                    }
                }
                continue;
            }

            let frame = frames.pop().ok_or(RuntimeError::Invariant(
                "unlinked function publication lost a completed frame",
            ))?;
            let root = self.finish_published_function(realm, frame)?;
            if let Some(parent) = frames.last_mut() {
                parent
                    .linked_constants
                    .push(BytecodeConstant::Function(root.bytecode_id()));
                parent.child_roots.push(root);
            } else {
                return Ok(root);
            }
        }
    }

    #[cfg(feature = "test262-host")]
    fn ensure_dynamic_import_bytecode_allowed(
        &self,
        code: &[crate::engine::code::bytecode::Instruction],
    ) -> Result<(), RuntimeError> {
        if self.0.dynamic_import_bytecode_allowed.get()
            || !code.iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::engine::code::bytecode::Instruction::Import
                )
            })
        {
            return Ok(());
        }
        Err(Error::internal("host dynamic-import bytecode policy rejected publication").into())
    }

    #[cfg(not(feature = "test262-host"))]
    fn ensure_dynamic_import_bytecode_allowed(
        &self,
        _code: &[crate::engine::code::bytecode::Instruction],
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn finish_published_function(
        &self,
        realm: ContextId,
        frame: PublishFrame<'_>,
    ) -> Result<FunctionBytecodeRef, RuntimeError> {
        let PublishFrame {
            code,
            remaining: _remaining,
            linked_constants,
            atom_string_constants,
            converted_constants,
            materialized_constant_roots,
            // The parent node owns children through its constant-pool edges
            // once allocation succeeds, so keep them rooted until then.
            child_roots: _child_roots,
            metadata,
            parameter_environment,
            func_name,
            argument_definitions,
            local_definitions,
            mut closure_variables,
            eval_environments,
            debug,
        } = frame;

        let mut linked_constants = linked_constants;
        // Guards for the constant-pool nodes created by this publication. The
        // bytecode node retains its own copies at allocation; until then these
        // keep the caller-owned producer edges alive through every failure
        // path.
        let mut converted_constants = converted_constants;
        let mut private_binding_scanner =
            bytecode_publish::PrivateBindingScanner::new(local_definitions.len());
        let mut private_local_roles = Vec::with_capacity(local_definitions.len());
        let mut private_closure_roles = Vec::with_capacity(closure_variables.len());
        let mut linked_argument_definitions = Vec::with_capacity(argument_definitions.len());
        let mut linked_local_definitions = Vec::with_capacity(local_definitions.len());
        let mut linked_eval_environments = Vec::with_capacity(eval_environments.len());
        let mut linked_private_bindings = PublishedPrivateBindings::none();
        let mut unlinked_debug = debug;
        let mut linked_debug = None;
        let mut auxiliary_atoms = Vec::new();
        let mut property_key_atoms = Vec::new();
        let id = {
            let mut state = self.0.state.borrow_mut();
            let linking = (|| -> Result<(), RuntimeError> {
                for (index, value) in atom_string_constants {
                    let atom = state.atoms.intern_property_key_js_string(&value)?;
                    // QuickJS falls back to an ordinary independent cpool
                    // String when JS_NewAtomStr produces a tagged integer.
                    let final_value = if atom.is_immediate_integer() {
                        value
                    } else {
                        auxiliary_atoms.push(atom);
                        state.atoms.to_js_string(atom)?
                    };
                    let string = state.heap.allocate_string(final_value)?;
                    // Only the final node is published. Its producer edge
                    // remains guarded until the bytecode retains its copy,
                    // including every later linking/publication failure.
                    converted_constants.push(ConvertedValue::new(self, RawValue::String(string)));
                    linked_constants[index] = BytecodeConstant::Value(RawValue::String(string));
                }
                property_key_atoms = bytecode_publish::link_constant_property_keys(
                    &mut state,
                    &code,
                    &linked_constants,
                    &mut auxiliary_atoms,
                )?;
                if let Some(debug) = unlinked_debug.take() {
                    let filename = state.atoms.intern_property_key_js_string(&debug.filename)?;
                    auxiliary_atoms.push(filename);
                    linked_debug = Some(FunctionDebugInfo {
                        filename,
                        pc2line: debug.pc2line,
                        source: debug.source,
                    });
                }
                for descriptor in &mut closure_variables {
                    let is_private = descriptor.kind.is_private();
                    let ClosureVariableName::Constant(index) = descriptor.name else {
                        if is_private {
                            return Err(RuntimeError::Invariant(
                                "verified private closure lost its unlinked source name",
                            ));
                        }
                        private_closure_roles.push(None);
                        continue;
                    };
                    let name = usize::try_from(index)
                        .ok()
                        .and_then(|index| linked_constants.get(index))
                        .and_then(|constant| match constant {
                            BytecodeConstant::Value(RawValue::String(name)) => Some(name),
                            BytecodeConstant::Value(_)
                            | BytecodeConstant::RegExp { .. }
                            | BytecodeConstant::Function(_) => None,
                        })
                        .ok_or(RuntimeError::Invariant(
                            "verified closure name was not a string constant",
                        ))?;
                    let text = state.heap.string(*name)?.clone();
                    let role = if is_private {
                        Some(bytecode_publish::private_closure_role(
                            descriptor.kind,
                            &text,
                        )?)
                    } else {
                        None
                    };
                    let atom = state.atoms.intern_property_key_js_string(&text)?;
                    auxiliary_atoms.push(atom);
                    descriptor.name = ClosureVariableName::Atom(atom);
                    private_closure_roles.push(role);
                }
                for definition in argument_definitions {
                    let name = definition
                        .name
                        .as_ref()
                        .map(|name| state.atoms.intern_property_key_js_string(name))
                        .transpose()?;
                    auxiliary_atoms.extend(name);
                    linked_argument_definitions.push(VariableDefinition {
                        name,
                        is_lexical: definition.is_lexical,
                        is_const: definition.is_const,
                        is_parameter_initializer: definition.is_parameter_initializer,
                        kind: definition.kind,
                    });
                }
                for (index, definition) in local_definitions.into_iter().enumerate() {
                    let role = private_binding_scanner.observe_local(
                        index,
                        definition.kind,
                        definition.name.as_ref(),
                    )?;
                    let name = definition
                        .name
                        .as_ref()
                        .map(|name| state.atoms.intern_property_key_js_string(name))
                        .transpose()?;
                    auxiliary_atoms.extend(name);
                    private_local_roles.push(role);
                    linked_local_definitions.push(VariableDefinition {
                        name,
                        is_lexical: definition.is_lexical,
                        is_const: definition.is_const,
                        is_parameter_initializer: definition.is_parameter_initializer,
                        kind: definition.kind,
                    });
                }
                private_binding_scanner.finish()?;
                if private_local_roles.iter().any(Option::is_some)
                    || private_closure_roles.iter().any(Option::is_some)
                {
                    let locals = private_local_roles
                        .into_iter()
                        .enumerate()
                        .map(|(index, role)| {
                            role.map(|role| {
                                let atom = linked_local_definitions[index].name.ok_or(
                                    RuntimeError::Invariant(
                                        "linked private local lost its atom name",
                                    ),
                                )?;
                                let pair = private_binding_scanner.pair_of(index);
                                Ok(match role {
                                    bytecode_publish::PrivateBindingRole::Primary => {
                                        PublishedPrivateBinding::primary(atom, pair)
                                    }
                                    bytecode_publish::PrivateBindingRole::SetterStorage => {
                                        PublishedPrivateBinding::setter_storage(atom, pair)
                                    }
                                })
                            })
                            .transpose()
                        })
                        .collect::<Result<Vec<_>, RuntimeError>>()?;
                    let closures = private_closure_roles
                        .into_iter()
                        .zip(&closure_variables)
                        .map(|(role, descriptor)| {
                            role.map(|role| {
                                let ClosureVariableName::Atom(atom) = descriptor.name else {
                                    return Err(RuntimeError::Invariant(
                                        "linked private closure lost its atom name",
                                    ));
                                };
                                Ok(match role {
                                    bytecode_publish::PrivateBindingRole::Primary => {
                                        PublishedPrivateBinding::primary(atom, None)
                                    }
                                    bytecode_publish::PrivateBindingRole::SetterStorage => {
                                        PublishedPrivateBinding::setter_storage(atom, None)
                                    }
                                })
                            })
                            .transpose()
                        })
                        .collect::<Result<Vec<_>, RuntimeError>>()?;
                    linked_private_bindings =
                        PublishedPrivateBindings::authenticated(locals, closures);
                }
                linked_eval_environments = bytecode_publish::link_eval_environments(
                    &mut state,
                    eval_environments,
                    &mut auxiliary_atoms,
                )?;
                Ok(())
            })();
            if let Err(error) = linking {
                state.release_atoms(auxiliary_atoms.drain(..))?;
                return Err(error);
            }

            let owned_atoms = auxiliary_atoms.clone();
            let bytecode = FunctionBytecodeData {
                executable: Default::default(),

                fusion: Default::default(),
                code: code.into(),
                constants: linked_constants.into(),
                property_key_atoms: (!property_key_atoms.is_empty())
                    .then(|| property_key_atoms.into()),
                realm,
                metadata,
                parameter_environment,
                func_name,
                argument_definitions: linked_argument_definitions.into(),
                local_definitions: linked_local_definitions.into(),
                closure_variables: closure_variables.into(),
                private_bindings: linked_private_bindings,
                eval_environments: linked_eval_environments.into(),
                debug: linked_debug,
                auxiliary_atoms: auxiliary_atoms.into_boxed_slice(),
            };
            match state.heap.allocate_function_bytecode(bytecode) {
                Ok(id) => id,
                Err(error) => {
                    state.release_atoms(owned_atoms)?;
                    return Err(error.into());
                }
            }
        };
        let root = FunctionBytecodeRef::from_owned_handle(self.clone(), id);
        // The bytecode node now owns every materialized template object
        // through its constant-pool RawValue edge.
        drop(materialized_constant_roots);
        Ok(root)
    }

    #[cfg(test)]
    pub fn test_function_debug_location(
        &self,
        function: &FunctionBytecodeRef,
        pc: Option<usize>,
    ) -> Result<Option<(JsString, LineColumn)>, RuntimeError> {
        if !function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        let state = self.0.state.borrow();
        let bytecode = state.heap.function_bytecode(function.bytecode_id())?;
        let Some(debug) = &bytecode.debug else {
            return Ok(None);
        };
        let filename = state.atoms.to_js_string(debug.filename)?;
        let position = debug
            .pc2line
            .as_ref()
            .map(|table| table.lookup(pc.and_then(|pc| u32::try_from(pc).ok())));
        Ok(position.map(|position| (filename, position)))
    }

    #[cfg(test)]
    pub fn test_function_debug_source(
        &self,
        function: &FunctionBytecodeRef,
    ) -> Result<Option<Vec<u8>>, RuntimeError> {
        if !function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        let state = self.0.state.borrow();
        Ok(state
            .heap
            .function_bytecode(function.bytecode_id())?
            .debug
            .as_ref()
            .and_then(|debug| debug.source.as_deref())
            .map(<[u8]>::to_vec))
    }

    #[cfg(test)]
    pub fn test_function_code(
        &self,
        function: &FunctionBytecodeRef,
    ) -> Result<Vec<crate::engine::code::bytecode::Instruction>, RuntimeError> {
        if !function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        Ok(self
            .0
            .state
            .borrow()
            .heap
            .function_bytecode(function.bytecode_id())?
            .code
            .to_vec())
    }

    #[cfg(test)]
    pub fn test_function_name(
        &self,
        function: &FunctionBytecodeRef,
    ) -> Result<Option<JsString>, RuntimeError> {
        if !function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        Ok(self
            .0
            .state
            .borrow()
            .heap
            .function_bytecode(function.bytecode_id())?
            .func_name
            .clone())
    }

    #[cfg(test)]
    pub fn test_debug_filename_atom_ownership(
        &self,
        function: &FunctionBytecodeRef,
    ) -> Result<Option<(usize, Option<u32>)>, RuntimeError> {
        if !function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        let state = self.0.state.borrow();
        let bytecode = state.heap.function_bytecode(function.bytecode_id())?;
        let Some(filename) = bytecode.debug.as_ref().map(|debug| debug.filename) else {
            return Ok(None);
        };
        let local_ownership = bytecode
            .auxiliary_atoms
            .iter()
            .filter(|atom| **atom == filename)
            .count();
        let total_ref_count = state.atoms.resolve(filename)?.ref_count;
        Ok(Some((local_ownership, total_ref_count)))
    }

    #[cfg(test)]
    pub fn test_atom_count(&self) -> usize {
        self.0.state.borrow().atoms.len()
    }

    #[cfg(test)]
    pub fn test_child_function_bytecode(
        &self,
        function: &FunctionBytecodeRef,
        constant_index: usize,
    ) -> Result<FunctionBytecodeRef, RuntimeError> {
        if !function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        let id = {
            let state = self.0.state.borrow();
            let bytecode = state.heap.function_bytecode(function.bytecode_id())?;
            match bytecode.constants.get(constant_index) {
                Some(BytecodeConstant::Function(id)) => *id,
                Some(BytecodeConstant::Value(_) | BytecodeConstant::RegExp { .. }) => {
                    return Err(RuntimeError::Invariant(
                        "requested child constant is a value",
                    ));
                }
                None => {
                    return Err(RuntimeError::Invariant(
                        "requested child constant is out of bounds",
                    ));
                }
            }
        };
        Ok(FunctionBytecodeRef::from_borrowed_handle(self.clone(), id)?)
    }
}

struct PublishFrame<'a> {
    code: Vec<crate::engine::code::bytecode::Instruction>,
    remaining: std::vec::IntoIter<UnlinkedConstant>,
    linked_constants: Vec<BytecodeConstant>,
    /// Atom-string constants deferred until canonicalization: the slot index
    /// points at the placeholder the linking pass replaces with the final
    /// arena node.
    atom_string_constants: Vec<(usize, JsString)>,
    /// Guards for arena nodes created while converting ordinary primitive
    /// constants during the publication walk.
    converted_constants: Vec<ConvertedValue<'a>>,
    materialized_constant_roots: Vec<ObjectRef>,
    child_roots: Vec<FunctionBytecodeRef>,
    metadata: FunctionMetadata,
    parameter_environment: Option<ParameterEnvironmentLayout>,
    func_name: Option<JsString>,
    argument_definitions: Vec<UnlinkedVariableDefinition>,
    local_definitions: Vec<UnlinkedVariableDefinition>,
    closure_variables: Vec<ClosureVariable>,
    eval_environments: Vec<EvalEnvironment<JsString>>,
    debug: Option<UnlinkedFunctionDebug>,
}

impl PublishFrame<'_> {
    fn new(function: UnlinkedFunction) -> Self {
        let parts = function.into_parts();
        let constants = parts.constants;
        let linked_constants = Vec::with_capacity(constants.len());
        Self {
            code: parts.code,
            remaining: constants.into_iter(),
            linked_constants,
            atom_string_constants: Vec::new(),
            converted_constants: Vec::new(),
            materialized_constant_roots: Vec::new(),
            child_roots: Vec::new(),
            metadata: parts.metadata,
            parameter_environment: parts.parameter_environment,
            func_name: parts.func_name,
            argument_definitions: parts.argument_definitions,
            local_definitions: parts.local_definitions,
            closure_variables: parts.closure_variables,
            eval_environments: parts.eval_environments,
            debug: parts.debug,
        }
    }
}
