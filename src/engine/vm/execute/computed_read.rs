//! Whole computed Get and ToPropKey entry; key callbacks use the same Query.
use super::FallthroughPc;
use crate::engine::{
    api::{Error, RuntimeError, error::ErrorKind, runtime::Runtime},
    heap::runtime::RuntimeState,
    value::{JsValue, conversion::property_key::PropertyKeyAtomStep},
    vm::{
        Completion, ToPrimitiveHint,
        exception::runtime_error_to_vm_error,
        proxy_get_driver::{
            Finish, RawNativeQuery, Resume, StateNativeProgress, Step, computed::ComputedRead,
        },
        stack::FrameExecution,
    },
};

pub(in crate::engine::vm) fn get(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    next_operation: &mut u64,
    keep_receiver: bool,
    keep_key: bool,
    fallthrough: FallthroughPc,
) -> Result<StateNativeProgress, Error> {
    let (realm, depth, object_key, nullish) = {
        let turn = segment.frame();
        let base = turn.transaction.peek(1)?;
        let key = turn.transaction.peek(0)?;
        let message = match base {
            JsValue::Null | JsValue::Undefined => Some(
                if keep_key
                    && !matches!(
                        key,
                        JsValue::Int(_) | JsValue::String(_) | JsValue::Symbol(_)
                    )
                {
                    "value has no property"
                } else if matches!(base, JsValue::Null) {
                    "cannot read property of null"
                } else {
                    "cannot read property of undefined"
                },
            ),
            _ => None,
        };
        (
            turn.executable.realm,
            turn.transaction.depth(),
            matches!(key, JsValue::Object(_)),
            message,
        )
    };
    if object_key && nullish.is_none() {
        *next_operation = next_operation
            .checked_add(1)
            .ok_or_else(|| Error::internal("property conversion identity exhausted"))?;
    }
    let input = ComputedRead::new(depth, fallthrough, keep_receiver, keep_key);
    let query = segment.acquire_read_query(realm, Finish::ComputedRead(input));
    let mut owner = RawNativeQuery::from_query(runtime, state, query, Step::Complete(None));
    let setup = (|| {
        if let Some(message) = nullish {
            owner.step = Step::ComputedError(Some(Error::new(ErrorKind::Type, message)));
        } else if object_key {
            let mut turn = segment.frame();
            let key = turn
                .transaction
                .slots()
                .pop()
                .expect("checked object key operand");
            let base = turn
                .transaction
                .slots()
                .pop()
                .expect("checked computed base operand");
            let input = owner
                .query
                .as_mut()
                .expect("computed query")
                .computed_read_mut()
                .expect("computed continuation");
            input.base = Some(base);
            input.consume = 0;
            owner.step = Step::Primitive {
                value: Some(key),
                hint: Some(ToPrimitiveHint::String),
                resume: Some(Resume::ComputedKey),
            };
        } else {
            let turn = segment.frame();
            let key = owner
                .state
                .dup_jsvalue(turn.transaction.peek(0)?)
                .map_err(runtime_error_to_vm_error)?;
            let atom = owner
                .state
                .property_key_from_primitive_jsvalue_with_publication(
                    &runtime.0.poisoned,
                    realm,
                    key,
                )
                .map_err(runtime_error_to_vm_error)?;
            match atom {
                PropertyKeyAtomStep::CyclePublishedThrow(value) => {
                    owner
                        .query
                        .as_mut()
                        .expect("computed query")
                        .carry_computed_publication();
                    owner.step = Step::Complete(Some(Completion::Throw(value)))
                }
                PropertyKeyAtomStep::Value(atom) => {
                    let input = owner
                        .query
                        .as_mut()
                        .expect("computed query")
                        .computed_read_mut()
                        .expect("computed continuation");
                    input.atom = Some(atom);
                    input
                        .preserve_key(owner.state, turn.transaction.peek(0)?, true)
                        .map_err(runtime_error_to_vm_error)?;
                    owner.step = input
                        .selected_step(
                            owner.state,
                            &runtime.0.poisoned,
                            runtime.domain_id(),
                            realm,
                            Some(turn.transaction.peek(1)?),
                        )
                        .map_err(runtime_error_to_vm_error)?;
                }
            }
        }
        Ok::<_, Error>(())
    })();
    if let Err(error) = setup {
        owner.retire().map_err(runtime_error_to_vm_error)?;
        return Err(error);
    }
    segment.consume_read_query(&mut owner, fallthrough)
}

pub(in crate::engine::vm) fn property_key(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    next_operation: &mut u64,
    fallthrough: FallthroughPc,
) -> Result<StateNativeProgress, Error> {
    let (realm, _depth) = {
        let turn = segment.frame();
        turn.transaction.peek(0)?;
        (turn.executable.realm, turn.transaction.depth())
    };
    // The old standalone instruction issues an identity for primitive inputs
    // too, before moving its one original input owner.
    *next_operation = next_operation
        .checked_add(1)
        .ok_or_else(|| Error::internal("conversion identity exhausted"))?;
    let query = segment.acquire_read_query(
        realm,
        Finish::PropertyKeyValue {
            #[cfg(feature = "profiling")]
            depth: _depth,
            fallthrough,
            cycle_published: false,
        },
    );
    let mut owner = RawNativeQuery::from_query(runtime, state, query, Step::Complete(None));
    let value = segment
        .frame()
        .transaction
        .slots()
        .pop()
        .expect("checked ToPropKey operand");
    owner.step = Step::Primitive {
        value: Some(value),
        hint: Some(ToPrimitiveHint::String),
        resume: Some(Resume::PropertyKeyValue),
    };
    segment.consume_read_query(&mut owner, fallthrough)
}

pub(super) fn progress(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    progress: StateNativeProgress,
) -> Result<super::named_read::Progress, Error> {
    Ok(match progress {
        StateNativeProgress::Published => super::named_read::Progress::Completed,
        StateNativeProgress::PublishedThrow => super::named_read::Progress::Throw,
        StateNativeProgress::Entered => super::named_read::Progress::Entered,
        StateNativeProgress::Boundary => super::named_read::Progress::NativeBoundary,
        StateNativeProgress::Complete(Completion::Throw(value)) => {
            let super::FrameTurn {
                transaction,
                fault_pc,
                resume_pc,
                ..
            } = segment.frame();
            let mut cursor =
                super::FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
            cursor.commit_owned(state, value)?;
            super::named_read::Progress::Throw
        }
        StateNativeProgress::Complete(Completion::Return(value)) => {
            state
                .release_owned_jsvalue(&runtime.0.poisoned, value)
                .map_err(runtime_error_to_vm_error)?;
            return Err(runtime_error_to_vm_error(RuntimeError::Invariant(
                "computed result bypassed publication",
            )));
        }
    })
}
