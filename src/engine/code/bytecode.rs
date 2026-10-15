use crate::engine::api::Error;
use std::collections::VecDeque;

/// QuickJS `JS_MAX_LOCAL_VARS` for one bytecode function.
pub const MAX_LOCAL_SLOTS: u16 = 65_534;

/// Ordinary-function arguments object selected by QuickJS's
/// `OP_special_object` lowering.
///
/// Simple sloppy parameter lists share their argument cells with indexed
/// properties; strict (and, once supported, non-simple) functions receive an
/// independent snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArgumentsKind {
    Mapped,
    Unmapped,
}

/// Storage which owns one hidden sloppy-eval variable object.
///
/// The object itself lives in a compiler-authenticated local or closure
/// `VarRef`. Keeping that source typed prevents the dynamic-name opcodes from
/// accepting an arbitrary JavaScript-visible object from the operand stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EvalVariableSource {
    Local(u16),
    Closure(u16),
}

/// Authenticated lexical storage for one class private name.
///
/// The private identity is deliberately never placed on the public operand
/// stack.  Dedicated class-element opcodes resolve it from a compiler-authored
/// local or closure cell after publication has verified the binding kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrivateNameSource {
    Local(u16),
    Closure(u16),
}

/// Storage which owns one hidden sloppy-`with` object.
///
/// Like [`EvalVariableSource`], this is compiler-authenticated frame state and
/// cannot be supplied by JavaScript through the operand stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WithObjectSource {
    Local(u16),
    Closure(u16),
}

/// One ordered object-environment record consulted by dynamic name lookup.
///
/// Keeping eval-variable and `with` sources distinct lets the host apply the
/// latter's `Symbol.unscopables` rules without weakening either source type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DynamicEnvironmentSource {
    Eval(EvalVariableSource),
    With(WithObjectSource),
}

/// Object-literal function role carried by QuickJS's `OP_define_method`
/// family.
///
/// The role is deliberately part of the verified instruction rather than a
/// JavaScript stack value: it selects both the function-name prefix and the
/// property descriptor shape used at publication time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DefineMethodKind {
    Method,
    Getter,
    Setter,
}

/// Typed flags for QuickJS `OP_iterator_call` in a `yield*` delegation loop.
/// The method lookup always targets the retained iterator object. Missing or
/// nullish methods preserve the input value and return a true missing flag;
/// present methods replace that value with their result and return false.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[expect(clippy::enum_variant_names)]
pub enum IteratorCallKind {
    ReturnWithValue,
    ThrowWithValue,
    ReturnWithoutValue,
}

/// Typed form of QuickJS's `OP_apply` magic operand.
///
/// Both forms consume `function, this-or-new-target, argument-array`; the
/// distinction is authenticated bytecode metadata rather than a
/// JavaScript-visible stack value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ApplyKind {
    Call,
    Construct,
}

pub use super::instruction::Instruction;

impl Instruction {
    /// String constant used as a static name by the VM's property-key bridge.
    /// Publication links these operands once, including unreachable code.
    #[must_use]
    pub(crate) const fn constant_property_key_index(&self) -> Option<u32> {
        self.operand_contract().static_name()
    }

    #[must_use]
    pub const fn stack_effect(&self) -> (usize, usize) {
        self.nominal_stack_effect()
    }
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub struct DetachedBytecode<T> {
    pub code: Vec<Instruction>,
    pub constants: Vec<T>,
    /// Declared local frame width. Local operands are verified against this
    /// boundary even when their instructions are unreachable.
    pub local_count: u16,
    pub max_stack: u16,
}

#[cfg(test)]
impl<T> DetachedBytecode<T> {
    /// Recompute the reachable stack depth against the declared maximum.
    ///
    /// # Errors
    /// Returns an internal error when the declared maximum is too small.
    pub fn verify(&self) -> Result<VerifiedBytecode, Error> {
        compute_stack_size(&self.code, self.max_stack)
    }
}

/// QuickJS `compute_stack_size`: the maximum operand-stack depth over every
/// reachable path, including exception and `gosub` edges. Fails once a
/// reachable depth exceeds `declared_max_stack`, which lowering reports as
/// QuickJS's "stack overflow".
///
/// Unwind regions are tracked only because `NipCatch` and the iterator
/// `*Preserve` cleanups truncate the stack to their region's base. The input
/// is compiler output; malformed bytecode is not diagnosed beyond the errors
/// needed to keep the walk bounds-safe.
pub fn compute_stack_size(
    code: &[Instruction],
    declared_max_stack: u16,
) -> Result<VerifiedBytecode, Error> {
    // Each region records the depth its abrupt cleanup restores: the catch
    // marker depth for `Catch`, or the iterator record base for for-of.
    let mut worklist = VecDeque::from([(0_usize, 0_usize, Vec::<usize>::new())]);
    // usize::MAX marks unvisited PCs; depths are bounded by MAX_STACK_SIZE.
    let mut visited = vec![usize::MAX; code.len()];
    let mut maximum = 0_usize;

    while let Some((pc, depth, mut regions)) = worklist.pop_front() {
        record_maximum_depth(&mut maximum, depth, declared_max_stack)?;
        let seen = visited
            .get_mut(pc)
            .ok_or_else(|| Error::internal("control flow target is out of bounds"))?;
        if *seen != usize::MAX {
            continue;
        }
        *seen = depth;

        let instruction = &code[pc];
        let effect = instruction.stack_contract();
        let remaining_depth = depth
            .checked_sub(effect.popped)
            .ok_or_else(|| Error::internal("bytecode stack underflow"))?;
        let mut next_depth = remaining_depth + effect.pushed;
        let missing_region = || Error::internal("bytecode unwind region underflow");
        match instruction {
            Instruction::DropCatch | Instruction::IteratorClose => {
                regions.pop().ok_or_else(missing_region)?;
            }
            Instruction::NipCatch => next_depth = regions.pop().ok_or_else(missing_region)?,
            Instruction::ForOfStart | Instruction::ForAwaitOfStart => {
                regions.push(remaining_depth);
            }
            Instruction::IteratorClosePreserve | Instruction::IteratorDropPreserve => {
                next_depth = regions.pop().ok_or_else(missing_region)? + 1;
            }
            Instruction::IteratorDetachPreserve => {
                next_depth = regions.pop().ok_or_else(missing_region)? + 2;
            }
            _ => {}
        }
        record_maximum_depth(&mut maximum, next_depth, declared_max_stack)?;
        // QuickJS `compute_stack_size` stops as soon as a reachable PC crosses
        // JS_STACK_SIZE_MAX, before any later instruction is considered.
        match *instruction {
            // QuickJS terminal completion opcodes consume their completion
            // value and abandon the rest of the frame stack. In particular,
            // `return` and `throw` inside a switch leave its discriminant
            // below that value rather than emitting synthetic cleanup.
            // `OP_throw_error` is likewise terminal: a postfix update may
            // retain its old value below the replaced immutable write.
            Instruction::TailCall(_)
            | Instruction::TailCallMethod(_)
            | Instruction::Return
            | Instruction::ReturnUndefined
            | Instruction::ReturnDerived(_)
            | Instruction::Throw
            | Instruction::Ret
            | Instruction::ThrowReadOnly(_)
            | Instruction::ThrowRedeclaration(_)
            | Instruction::ThrowDeleteSuper
            | Instruction::ThrowIteratorMissingThrow => {}
            Instruction::Goto(target) => worklist.push_back((target as usize, next_depth, regions)),
            Instruction::IfFalse(target) | Instruction::IfTrue(target) => {
                worklist.push_back((target as usize, next_depth, regions.clone()));
                worklist.push_back((pc + 1, next_depth, regions));
            }
            Instruction::Catch(target) => {
                worklist.push_back((target as usize, next_depth, regions.clone()));
                regions.push(next_depth);
                worklist.push_back((pc + 1, next_depth, regions));
            }
            Instruction::Gosub(target) => {
                let subroutine_depth = depth + 1;
                record_maximum_depth(&mut maximum, subroutine_depth, declared_max_stack)?;
                worklist.push_back((target as usize, subroutine_depth, regions.clone()));
                worklist.push_back((pc + 1, next_depth, regions));
            }
            _ => worklist.push_back((pc + 1, next_depth, regions)),
        }
    }

    let maximum =
        u16::try_from(maximum).map_err(|_| Error::internal("bytecode stack exceeds u16::MAX"))?;
    Ok(VerifiedBytecode { max_stack: maximum })
}

fn record_maximum_depth(
    maximum: &mut usize,
    depth: usize,
    declared_max_stack: u16,
) -> Result<(), Error> {
    *maximum = (*maximum).max(depth);
    if *maximum > usize::from(declared_max_stack) {
        return Err(Error::internal(
            "declared maximum stack is smaller than required",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedBytecode {
    pub max_stack: u16,
}

#[cfg(test)]
impl<T> Default for DetachedBytecode<T> {
    fn default() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            local_count: 0,
            max_stack: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApplyKind, ArgumentsKind, DefineMethodKind, DetachedBytecode, DynamicEnvironmentSource,
        EvalVariableSource, Instruction, WithObjectSource,
    };
    use crate::engine::api::JsString;
    use crate::engine::value::PrimitiveValue as Value;

    #[test]
    fn verifier_computes_reachable_stack_depth() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushTrue,
                Instruction::IfFalse(4),
                Instruction::PushI32(1),
                Instruction::Goto(5),
                Instruction::PushI32(2),
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(function.verify().unwrap().max_stack, 1);
    }

    #[test]
    fn verifier_models_generator_suspension_resume_stack_shapes() {
        for suspension in [
            Instruction::Yield,
            Instruction::YieldStar,
            Instruction::AsyncYieldStar,
        ] {
            let function = DetachedBytecode::<Value> {
                code: vec![
                    Instruction::InitialYield,
                    Instruction::PushI32(42),
                    suspension,
                    Instruction::IfFalse(5),
                    Instruction::Return,
                    Instruction::Return,
                ],
                constants: vec![],
                local_count: 0,
                max_stack: 2,
            };
            assert_eq!(function.verify().unwrap().max_stack, 2);
        }

        let underflow = DetachedBytecode::<Value> {
            code: vec![Instruction::Yield, Instruction::Return],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(
            underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );
    }

    #[test]
    fn verifier_models_await_as_a_value_preserving_suspension() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(42),
                Instruction::Await,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(function.verify().unwrap().max_stack, 1);

        let underflow = DetachedBytecode::<Value> {
            code: vec![Instruction::Await, Instruction::Return],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(
            underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );
    }

    #[test]
    fn verifier_models_dynamic_import_as_two_inputs_to_one_promise() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(20),
                Instruction::PushI32(22),
                Instruction::Import,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(function.verify().unwrap().max_stack, 2);

        let underflow = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(20),
                Instruction::Import,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(
            underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );

        let undersized = DetachedBytecode::<Value> {
            max_stack: 1,
            ..function
        };
        assert_eq!(
            undersized.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );
    }

    #[test]
    fn verifier_models_typed_class_definition_operations() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::CheckCtor,
                Instruction::Undefined,
                Instruction::PushI32(7),
                Instruction::Swap,
                Instruction::Swap,
                Instruction::DefineClass {
                    name: 0,
                    has_heritage: false,
                },
                Instruction::Nip,
                Instruction::Return,
            ],
            constants: vec![Value::String(crate::engine::api::JsString::from_static(
                "C",
            ))],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(function.verify().unwrap().max_stack, 2);

        let underflow = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::DefineClass {
                    name: 0,
                    has_heritage: false,
                },
                Instruction::Return,
            ],
            max_stack: 2,
            ..function.clone()
        };
        assert_eq!(
            underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );

        let heritage = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::PushI32(7),
                Instruction::DefineClass {
                    name: 0,
                    has_heritage: true,
                },
                Instruction::Drop,
                Instruction::Return,
            ],
            constants: vec![Value::String(crate::engine::api::JsString::from_static(
                "Derived",
            ))],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(heritage.verify().unwrap().max_stack, 2);
    }

    #[test]
    fn verifier_models_arguments_creation_as_one_fresh_value() {
        for kind in [ArgumentsKind::Mapped, ArgumentsKind::Unmapped] {
            let function = DetachedBytecode::<Value> {
                code: vec![Instruction::Arguments(kind), Instruction::Return],
                constants: vec![],
                local_count: 0,
                max_stack: 1,
            };
            assert_eq!(function.verify().unwrap().max_stack, 1);
        }

        let undersized = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Arguments(ArgumentsKind::Mapped),
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 0,
        };
        assert_eq!(
            undersized.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );

        let rest = DetachedBytecode::<Value> {
            code: vec![Instruction::Rest(3), Instruction::Return],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(rest.verify().unwrap().max_stack, 1);

        let undersized_rest = DetachedBytecode::<Value> {
            max_stack: 0,
            ..rest
        };
        assert_eq!(
            undersized_rest.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );
    }

    #[test]
    fn verifier_models_tail_invocations_as_terminal_zero_result_operations() {
        for (code, maximum) in [
            (
                vec![
                    Instruction::Undefined,
                    Instruction::PushI32(1),
                    Instruction::PushI32(2),
                    Instruction::TailCall(2),
                    // Terminal tail invocation makes this underflowing
                    // instruction unreachable.
                    Instruction::Drop,
                ],
                3,
            ),
            (
                vec![
                    Instruction::PushI32(9),
                    Instruction::Undefined,
                    Instruction::PushI32(1),
                    Instruction::PushI32(2),
                    Instruction::TailCallMethod(2),
                    Instruction::Drop,
                ],
                4,
            ),
        ] {
            let function = DetachedBytecode::<Value> {
                code,
                constants: vec![],
                local_count: 0,
                max_stack: maximum,
            };
            assert_eq!(function.verify().unwrap().max_stack, maximum);
        }

        for instruction in [Instruction::TailCall(0), Instruction::TailCallMethod(0)] {
            let function = DetachedBytecode::<Value> {
                code: vec![instruction],
                constants: vec![],
                local_count: 0,
                max_stack: 0,
            };
            assert_eq!(
                function.verify().unwrap_err().message(),
                "bytecode stack underflow"
            );
        }

        let undersized = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::PushI32(1),
                Instruction::PushI32(2),
                Instruction::TailCall(2),
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(
            undersized.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );

        for instruction in [
            Instruction::TailCall(u16::MAX),
            Instruction::TailCallMethod(u16::MAX),
        ] {
            let function = DetachedBytecode::<Value> {
                code: vec![instruction],
                constants: vec![],
                local_count: 0,
                max_stack: 0,
            };
            assert_eq!(
                function.verify().unwrap_err().message(),
                "bytecode stack underflow"
            );
        }
    }

    #[test]
    fn verifier_models_eval_with_the_quickjs_call_stack_shape() {
        for (argument_count, code, maximum) in [
            (
                0,
                vec![
                    Instruction::Undefined,
                    Instruction::Eval {
                        argument_count: 0,
                        environment: 0,
                    },
                    Instruction::Return,
                ],
                1,
            ),
            (
                2,
                vec![
                    Instruction::Undefined,
                    Instruction::PushI32(1),
                    Instruction::PushI32(2),
                    Instruction::Eval {
                        argument_count: 2,
                        environment: 0,
                    },
                    Instruction::Return,
                ],
                3,
            ),
        ] {
            let function = DetachedBytecode::<Value> {
                code,
                constants: vec![],
                local_count: 0,
                max_stack: maximum,
            };
            assert_eq!(
                function.verify().unwrap().max_stack,
                maximum,
                "argument count {argument_count}"
            );
        }

        for code in [
            vec![
                Instruction::Eval {
                    argument_count: 0,
                    environment: 0,
                },
                Instruction::Return,
            ],
            vec![
                Instruction::Undefined,
                Instruction::PushI32(1),
                Instruction::Eval {
                    argument_count: 2,
                    environment: 0,
                },
                Instruction::Return,
            ],
        ] {
            let function = DetachedBytecode::<Value> {
                code,
                constants: vec![],
                local_count: 0,
                max_stack: 2,
            };
            assert_eq!(
                function.verify().unwrap_err().message(),
                "bytecode stack underflow"
            );
        }

        let undersized = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::PushI32(1),
                Instruction::PushI32(2),
                Instruction::Eval {
                    argument_count: 2,
                    environment: 0,
                },
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(
            undersized.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );
    }

    #[test]
    fn verifier_models_apply_and_apply_eval_stack_shapes() {
        for (code, maximum) in [
            (
                vec![
                    Instruction::Undefined,
                    Instruction::Undefined,
                    Instruction::Undefined,
                    Instruction::Apply(ApplyKind::Call),
                    Instruction::Return,
                ],
                3,
            ),
            (
                vec![
                    Instruction::Undefined,
                    Instruction::Undefined,
                    Instruction::Undefined,
                    Instruction::Apply(ApplyKind::Construct),
                    Instruction::Return,
                ],
                3,
            ),
            (
                vec![
                    Instruction::Undefined,
                    Instruction::Undefined,
                    Instruction::ApplyEval { environment: 7 },
                    Instruction::Return,
                ],
                2,
            ),
        ] {
            let function = DetachedBytecode::<Value> {
                code,
                constants: vec![],
                local_count: 0,
                max_stack: maximum,
            };
            assert_eq!(function.verify().unwrap().max_stack, maximum);
        }

        for instruction in [
            Instruction::Apply(ApplyKind::Call),
            Instruction::Apply(ApplyKind::Construct),
            Instruction::ApplyEval { environment: 0 },
        ] {
            let function = DetachedBytecode::<Value> {
                code: vec![Instruction::Undefined, instruction, Instruction::Return],
                constants: vec![],
                local_count: 0,
                max_stack: 1,
            };
            assert_eq!(
                function.verify().unwrap_err().message(),
                "bytecode stack underflow"
            );
        }
    }

    #[test]
    fn verifier_models_membership_operators_as_binary_booleans() {
        for operator in [Instruction::InstanceOf, Instruction::In] {
            let valid = DetachedBytecode::<Value> {
                code: vec![
                    Instruction::PushI32(1),
                    Instruction::PushI32(2),
                    operator.clone(),
                    Instruction::Return,
                ],
                constants: vec![],
                local_count: 0,
                max_stack: 2,
            };
            assert_eq!(valid.verify().unwrap().max_stack, 2);

            let underflow = DetachedBytecode::<Value> {
                code: vec![Instruction::PushI32(1), operator, Instruction::Return],
                constants: vec![],
                local_count: 0,
                max_stack: 1,
            };
            assert!(underflow.verify().is_err());
        }
    }

    #[test]
    fn verifier_models_eval_variable_object_stack_and_operands() {
        let source = EvalVariableSource::Local(0);
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::VariableEnvironment,
                Instruction::PutLocal(0),
                Instruction::HasEvalVariable { source, name: 0 },
                Instruction::Drop,
                Instruction::GetEvalVariable { source, name: 0 },
                Instruction::Drop,
                Instruction::PushI32(1),
                Instruction::PutEvalVariable { source, name: 0 },
                Instruction::DeleteEvalVariable { source, name: 0 },
                Instruction::Drop,
                Instruction::PushI32(2),
                Instruction::DefineEvalVariable { source, name: 0 },
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![Value::String(JsString::from_static("added"))],
            local_count: 1,
            max_stack: 1,
        };
        assert_eq!(function.verify().unwrap().max_stack, 1);
    }

    #[test]
    fn verifier_models_typed_dynamic_environment_stack_and_operands() {
        let eval = DynamicEnvironmentSource::Eval(EvalVariableSource::Local(0));
        let with = DynamicEnvironmentSource::With(WithObjectSource::Local(1));
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::VariableEnvironment,
                Instruction::PutLocal(0),
                Instruction::PushI32(1),
                Instruction::ToObject,
                Instruction::PutLocal(1),
                Instruction::HasDynamicBinding {
                    source: eval,
                    name: 0,
                },
                Instruction::Drop,
                Instruction::GetDynamicBinding {
                    source: with,
                    name: 0,
                },
                Instruction::Drop,
                Instruction::PushI32(2),
                Instruction::PutDynamicBinding {
                    source: eval,
                    name: 0,
                },
                Instruction::DeleteDynamicBinding {
                    source: with,
                    name: 0,
                },
                Instruction::Drop,
                Instruction::DynamicEnvironmentObject(eval),
                Instruction::GetRefValue(0),
                Instruction::PutRefValue(0),
                Instruction::DynamicEnvironmentObject(with),
                Instruction::GetRefValueUndef(0),
                Instruction::PutRefValue(0),
                Instruction::GlobalReference(0),
                Instruction::Drop,
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![Value::String(JsString::from_static("binding"))],
            local_count: 2,
            max_stack: 2,
        };
        assert_eq!(function.verify().unwrap().max_stack, 2);

        let undersized_global_reference = DetachedBytecode::<Value> {
            code: vec![Instruction::GlobalReference(0), Instruction::Return],
            constants: vec![],
            local_count: 0,
            max_stack: 0,
        };
        assert_eq!(
            undersized_global_reference.verify().unwrap_err().message(),
            "declared maximum stack is smaller than required"
        );
    }

    #[test]
    fn verifier_models_eval_redeclaration_as_a_zero_stack_terminal() {
        let function = DetachedBytecode::<Value> {
            code: vec![Instruction::ThrowRedeclaration(0)],
            constants: vec![Value::String(JsString::from_static("conflict"))],
            local_count: 0,
            max_stack: 0,
        };
        assert_eq!(function.verify().unwrap().max_stack, 0);
    }

    #[test]
    fn verifier_accepts_closed_non_terminating_control_flow() {
        let cycle = DetachedBytecode::<Value> {
            code: vec![Instruction::Goto(0)],
            constants: vec![],
            local_count: 0,
            max_stack: 0,
        };
        assert_eq!(cycle.verify().unwrap().max_stack, 0);
    }

    #[test]
    fn verifier_allows_terminal_completion_to_abandon_switch_values() {
        for completion in [Instruction::Return, Instruction::Throw] {
            let function = DetachedBytecode::<Value> {
                code: vec![Instruction::PushI32(1), Instruction::PushI32(2), completion],
                constants: vec![],
                local_count: 0,
                max_stack: 2,
            };
            assert_eq!(function.verify().unwrap().max_stack, 2);
        }
    }

    #[test]
    fn verifier_tracks_catch_markers_and_exceptional_edges() {
        let normal = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Catch(4),
                Instruction::DropCatch,
                Instruction::PushI32(3),
                Instruction::Return,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(normal.verify().unwrap().max_stack, 1);

        let thrown = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Catch(3),
                Instruction::PushI32(7),
                Instruction::Throw,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(thrown.verify().unwrap().max_stack, 2);

        let nip = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(10),
                Instruction::Catch(6),
                Instruction::PushI32(20),
                Instruction::PushI32(30),
                Instruction::NipCatch,
                Instruction::Return,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(nip.verify().unwrap().max_stack, 4);

        let marker_exceeds_declared_stack = DetachedBytecode::<Value> {
            max_stack: 0,
            ..normal
        };
        assert!(marker_exceeds_declared_stack.verify().is_err());
    }

    #[test]
    fn verifier_tracks_typed_gosub_return_addresses() {
        let returning = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(9),
                Instruction::Gosub(4),
                Instruction::Return,
                Instruction::Nop,
                Instruction::Ret,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(returning.verify().unwrap().max_stack, 2);

        let abrupt_cleanup = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(9),
                Instruction::Gosub(4),
                Instruction::Return,
                Instruction::Nop,
                Instruction::DropGosub,
                Instruction::Drop,
                Instruction::PushI32(4),
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(abrupt_cleanup.verify().unwrap().max_stack, 2);

        let return_address_exceeds_declared_stack = DetachedBytecode::<Value> {
            max_stack: 1,
            ..returning
        };
        assert!(return_address_exceeds_declared_stack.verify().is_err());
    }

    #[test]
    fn verifier_dynamic_cleanup_discards_intermediate_gosub_return_addresses() {
        // A return from a finally subroutine can override the pending
        // completion protected by this catch. NipCatch preserves the new
        // return value and intentionally truncates the now-obsolete Gosub PC.
        let nip_catch = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Catch(10),
                Instruction::Undefined,
                Instruction::Gosub(7),
                Instruction::Drop,
                Instruction::DropCatch,
                Instruction::Undefined,
                Instruction::Return,
                Instruction::PushI32(7),
                Instruction::NipCatch,
                Instruction::Return,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(nip_catch.verify().unwrap().max_stack, 4);

        // The same dynamic truncation is required when a return crosses an
        // iterator region while a nested finally Gosub is active.
        let iterator_close = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::Gosub(6),
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
                Instruction::PushI32(9),
                Instruction::IteratorClosePreserve,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 5,
        };
        assert_eq!(iterator_close.verify().unwrap().max_stack, 5);
    }

    #[test]
    fn verifier_tracks_for_of_records_offsets_and_dynamic_close() {
        let normal = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::ForOfNext(0),
                Instruction::Drop,
                Instruction::Drop,
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 5,
        };
        assert_eq!(normal.verify().unwrap().max_stack, 5);

        let offset = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::PushI32(2),
                Instruction::ForOfNext(1),
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 6,
        };
        assert_eq!(offset.verify().unwrap().max_stack, 6);

        let preserve = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::PushI32(42),
                Instruction::IteratorClosePreserve,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(preserve.verify().unwrap().max_stack, 4);

        let drop_preserve = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::PushI32(42),
                Instruction::IteratorDropPreserve,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(drop_preserve.verify().unwrap().max_stack, 4);
    }

    #[test]
    fn verifier_tracks_for_await_records_phases_and_dynamic_detach() {
        let normal = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForAwaitOfStart,
                Instruction::ForAwaitOfNext,
                Instruction::Await,
                Instruction::IteratorGetValueDone,
                Instruction::Drop,
                Instruction::Drop,
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 5,
        };
        assert_eq!(normal.verify().unwrap().max_stack, 5);

        for start in [Instruction::ForOfStart, Instruction::ForAwaitOfStart] {
            let detach = DetachedBytecode::<Value> {
                code: vec![
                    Instruction::PushI32(1),
                    start,
                    Instruction::PushI32(42),
                    Instruction::IteratorDetachPreserve,
                    // Detach leaves the iterator above the preserved
                    // completion, ready for explicit close lowering.
                    Instruction::Drop,
                    Instruction::Return,
                ],
                constants: vec![],
                local_count: 0,
                max_stack: 4,
            };
            assert_eq!(detach.verify().unwrap().max_stack, 4);
        }
    }

    #[test]
    fn verifier_tracks_for_in_as_one_ordinary_retained_operand() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForInStart,
                Instruction::ForInNext,
                Instruction::IfFalse(8),
                Instruction::Drop,
                Instruction::Drop,
                Instruction::Undefined,
                Instruction::Return,
                Instruction::Drop,
                Instruction::Goto(2),
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(function.verify().unwrap().max_stack, 3);

        let underflow = DetachedBytecode::<Value> {
            code: vec![Instruction::ForInStart, Instruction::Return],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert!(underflow.verify().is_err());
    }

    #[test]
    fn verifier_orders_iterator_catch_and_gosub_regions() {
        // The iterator is inside a catch region. Preserving the return value
        // closes it first, then NipCatch removes the surrounding catch marker.
        let iterator_inside_catch = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Catch(7),
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::PushI32(9),
                Instruction::IteratorClosePreserve,
                Instruction::NipCatch,
                Instruction::Return,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 5,
        };
        assert_eq!(iterator_inside_catch.verify().unwrap().max_stack, 5);

        // The catch is inside an iterator. Its exceptional edge retains the
        // outer iterator region, which is then explicitly closed.
        let catch_inside_iterator = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::Catch(7),
                Instruction::DropCatch,
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
                Instruction::Drop,
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(catch_inside_iterator.verify().unwrap().max_stack, 4);

        // A finally return PC is below the iterator record. Closing the record
        // must leave that typed address at TOS for Ret.
        let iterator_above_outer_gosub = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::Gosub(5),
                Instruction::Return,
                Instruction::Nop,
                Instruction::Nop,
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::IteratorClose,
                Instruction::Ret,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 5,
        };
        assert_eq!(iterator_above_outer_gosub.verify().unwrap().max_stack, 5);

        // Conversely a gosub may temporarily put its address above an outer
        // iterator marker, provided Ret consumes it before iterator cleanup.
        let gosub_above_iterator = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::ForOfStart,
                Instruction::Gosub(6),
                Instruction::IteratorClose,
                Instruction::Undefined,
                Instruction::Return,
                Instruction::Ret,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(gosub_above_iterator.verify().unwrap().max_stack, 4);
    }

    #[test]
    fn verifier_models_quickjs_lexical_frame_operations() {
        let initialized_then_updated = DetachedBytecode::<Value> {
            code: vec![
                Instruction::SetLocalUninitialized(0),
                Instruction::PushI32(1),
                Instruction::InitializeLocal(0),
                Instruction::GetLocalCheck(0),
                Instruction::PushI32(1),
                Instruction::Add,
                Instruction::SetLocalCheck(0),
                Instruction::CloseLocal(0),
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 1,
            max_stack: 2,
        };
        assert_eq!(initialized_then_updated.verify().unwrap().max_stack, 2);

        let consuming_write = DetachedBytecode::<Value> {
            code: vec![
                Instruction::SetLocalUninitialized(0),
                Instruction::PushI32(1),
                Instruction::InitializeLocal(0),
                Instruction::PushI32(2),
                Instruction::PutLocalCheck(0),
                Instruction::GetLocalCheck(0),
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 1,
            max_stack: 1,
        };
        assert_eq!(consuming_write.verify().unwrap().max_stack, 1);

        let checked_var_ref = DetachedBytecode::<Value> {
            code: vec![
                Instruction::GetVarRefCheck(0),
                Instruction::PutVarRefCheck(0),
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(checked_var_ref.verify().unwrap().max_stack, 1);

        for instruction in [
            Instruction::InitializeLocal(0),
            Instruction::InitializeDerivedLocal(0),
            Instruction::PutLocalCheck(0),
            Instruction::SetLocalCheck(0),
            Instruction::PutVarRefCheck(0),
            Instruction::InitializeDerivedVarRef(0),
        ] {
            let underflow = DetachedBytecode::<Value> {
                code: vec![instruction, Instruction::Undefined, Instruction::Return],
                constants: vec![],
                local_count: 1,
                max_stack: 1,
            };
            assert!(underflow.verify().is_err());
        }
    }

    #[test]
    fn verifier_models_typed_derived_constructor_operations() {
        let default_derived = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushActiveFunction,
                Instruction::PutLocal(1),
                Instruction::InitDerivedConstructor,
                Instruction::Dup,
                Instruction::InitializeDerivedLocal(0),
                Instruction::ReturnDerived(0),
            ],
            constants: vec![],
            local_count: 2,
            max_stack: 2,
        };
        assert_eq!(default_derived.verify().unwrap().max_stack, 2);

        let captured_initializer = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::InitializeDerivedVarRef(0),
                Instruction::Undefined,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 1,
        };
        assert_eq!(captured_initializer.verify().unwrap().max_stack, 1);

        let return_underflow = DetachedBytecode::<Value> {
            code: vec![Instruction::ReturnDerived(0)],
            local_count: 1,
            max_stack: 0,
            ..DetachedBytecode::<Value>::default()
        };
        assert_eq!(
            return_underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );

        // ReturnDerived is a terminal completion and must not make its
        // unreachable successor part of the control-flow graph.
        let terminal = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Undefined,
                Instruction::ReturnDerived(0),
                Instruction::PushI32(1),
            ],
            local_count: 1,
            max_stack: 1,
            ..DetachedBytecode::<Value>::default()
        };
        assert_eq!(terminal.verify().unwrap().max_stack, 1);
    }

    #[test]
    fn verifier_models_array_literal_stack_contracts() {
        let fixed = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::PushI32(2),
                Instruction::ArrayFrom(2),
                Instruction::PushI32(3),
                Instruction::DefineField(0),
                Instruction::Return,
            ],
            constants: vec![Value::String(crate::engine::value::JsString::from_static(
                "2",
            ))],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(fixed.verify().unwrap().max_stack, 2);

        let dynamic = DetachedBytecode::<Value> {
            code: vec![
                Instruction::ArrayFrom(0),
                Instruction::PushI32(0),
                Instruction::Undefined,
                Instruction::DefineArrayEl,
                Instruction::Inc,
                Instruction::Undefined,
                Instruction::Append,
                Instruction::Drop,
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(dynamic.verify().unwrap().max_stack, 3);

        let trailing_hole = DetachedBytecode::<Value> {
            code: vec![
                Instruction::ArrayFrom(0),
                Instruction::PushI32(2),
                Instruction::Dup1,
                Instruction::PutField(0),
                Instruction::Return,
            ],
            constants: vec![Value::String(crate::engine::value::JsString::from_static(
                "length",
            ))],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(trailing_hole.verify().unwrap().max_stack, 3);
    }

    #[test]
    fn verifier_models_object_literal_stack_contracts() {
        let fixed_proto_spread = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Object,
                Instruction::PushI32(1),
                Instruction::DefineField(0),
                Instruction::Undefined,
                Instruction::DefineMethod {
                    key: 1,
                    kind: DefineMethodKind::Method,
                    enumerable: true,
                },
                Instruction::Null,
                Instruction::SetProto,
                Instruction::Undefined,
                Instruction::CopyDataProperties,
                Instruction::Return,
            ],
            constants: vec![
                Value::String(crate::engine::value::JsString::from_static("field")),
                Value::String(crate::engine::value::JsString::from_static("method")),
            ],
            local_count: 0,
            max_stack: 2,
        };
        assert_eq!(fixed_proto_spread.verify().unwrap().max_stack, 2);

        let computed = DetachedBytecode::<Value> {
            code: vec![
                Instruction::Object,
                Instruction::PushI32(1),
                Instruction::ToPropKey,
                Instruction::Undefined,
                Instruction::DefineMethodComputed {
                    kind: DefineMethodKind::Getter,
                    enumerable: true,
                },
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(computed.verify().unwrap().max_stack, 3);
    }

    #[test]
    fn verifier_models_depth_addressed_object_rest_copy_without_stack_mutation() {
        let function = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1), // excluded
                Instruction::PushI32(2), // source
                Instruction::PushI32(3), // prepared Reference
                Instruction::PushI32(4), // target
                Instruction::CopyDataPropertiesExcluded {
                    target_depth: 0,
                    source_depth: 2,
                    excluded_depth: 3,
                },
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 4,
        };
        assert_eq!(function.verify().unwrap().max_stack, 4);

        let underflow = DetachedBytecode::<Value> {
            code: vec![
                Instruction::PushI32(1),
                Instruction::PushI32(2),
                Instruction::PushI32(3),
                Instruction::CopyDataPropertiesExcluded {
                    target_depth: 0,
                    source_depth: 2,
                    excluded_depth: 3,
                },
                Instruction::Return,
            ],
            constants: vec![],
            local_count: 0,
            max_stack: 3,
        };
        assert_eq!(
            underflow.verify().unwrap_err().message(),
            "bytecode stack underflow"
        );
    }

    #[test]
    fn verifier_rejects_object_literal_opcode_underflow() {
        for instruction in [
            Instruction::SetNameComputed,
            Instruction::DefineMethod {
                key: 0,
                kind: DefineMethodKind::Method,
                enumerable: true,
            },
            Instruction::DefineMethodComputed {
                kind: DefineMethodKind::Setter,
                enumerable: true,
            },
            Instruction::SetProto,
            Instruction::CopyDataProperties,
        ] {
            let constants = if matches!(&instruction, Instruction::DefineMethod { .. }) {
                vec![Value::String(crate::engine::value::JsString::from_static(
                    "method",
                ))]
            } else {
                vec![]
            };
            let function = DetachedBytecode::<Value> {
                code: vec![instruction, Instruction::Return],
                constants,
                local_count: 0,
                max_stack: 2,
            };
            assert_eq!(
                function.verify().unwrap_err().message(),
                "bytecode stack underflow"
            );
        }
    }
}
