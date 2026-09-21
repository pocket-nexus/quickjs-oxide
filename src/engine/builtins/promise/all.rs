//! QuickJS's shared `Promise.all`/`allSettled`/`any` aggregate loop.
//!
//! The entry algorithm is shared, but each callback family keeps a distinct
//! typed heap capture. In particular, pinned QuickJS copies `allSettled`'s
//! fulfill and reject CFunctionData records, so their first-call bits are
//! deliberately independent rather than one shared specification record.

use std::{cell::Cell, rc::Rc};

use super::*;
use crate::engine::object::{OwnedPropertyDescriptor, operations::PropertyDefineOutcome};

#[derive(Clone, Copy)]
enum AggregateTerminal {
    ResolveValues,
    RejectAggregate,
}

impl Runtime {
    pub(crate) fn call_promise_aggregate(
        &self,
        kind: PromiseNativeKind,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            operation::PromiseStep::aggregate(self, realm, kind, invocation, arguments)?
                .finish(self, realm)
        })
    }

    pub(super) fn prepare_promise_aggregate_handlers(
        &self,
        realm: ContextId,
        kind: PromiseNativeKind,
        values: &ObjectRef,
        capability: &RootedPromiseCapability,
        remaining: &Rc<Cell<i32>>,
        index: u32,
    ) -> Result<NativeConversion<[JsValue; 2]>, RuntimeError> {
        let then_arguments = match kind {
            PromiseNativeKind::All => {
                let resolve_element = self.new_internal_promise_function(
                    realm,
                    NativeFunctionId::PromiseAllResolveElement,
                    1,
                    1,
                    InternalCallableData::PromiseAllResolveElement {
                        values: values.object_id(),
                        resolve: capability.resolve.as_object().object_id(),
                        remaining: remaining.clone(),
                        already_called: Rc::new(Cell::new(false)),
                        index,
                    },
                )?;
                [
                    JsValue::Object(resolve_element.as_object().clone().into_handle()),
                    JsValue::Object(capability.reject.as_object().clone().into_handle()),
                ]
            }
            PromiseNativeKind::AllSettled => {
                let make_element = |outcome| {
                    self.new_internal_promise_function(
                        realm,
                        NativeFunctionId::PromiseAllSettledElement(outcome),
                        1,
                        1,
                        InternalCallableData::PromiseAllSettledElement {
                            values: values.object_id(),
                            resolve: capability.resolve.as_object().object_id(),
                            remaining: remaining.clone(),
                            already_called: Rc::new(Cell::new(false)),
                            index,
                            outcome,
                        },
                    )
                };
                let fulfill_element = make_element(PromiseReactionKind::Fulfill)?;
                let reject_element = make_element(PromiseReactionKind::Reject)?;
                [
                    JsValue::Object(fulfill_element.as_object().clone().into_handle()),
                    JsValue::Object(reject_element.as_object().clone().into_handle()),
                ]
            }
            PromiseNativeKind::Any => {
                let reject_element = self.new_internal_promise_function(
                    realm,
                    NativeFunctionId::PromiseAnyRejectElement,
                    1,
                    1,
                    InternalCallableData::PromiseAnyRejectElement {
                        errors: values.object_id(),
                        reject: capability.reject.as_object().object_id(),
                        remaining: remaining.clone(),
                        already_called: Rc::new(Cell::new(false)),
                        index,
                    },
                )?;
                if let Some(value) =
                    self.define_promise_array_element(realm, values, index, &JsValue::Undefined)?
                {
                    return Ok(NativeConversion::Throw(value));
                }
                [
                    JsValue::Object(capability.resolve.as_object().clone().into_handle()),
                    JsValue::Object(reject_element.as_object().clone().into_handle()),
                ]
            }
            _ => unreachable!("aggregate selector was validated above"),
        };
        Ok(NativeConversion::Value(then_arguments))
    }

    pub(crate) fn call_promise_all_resolve_element(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            self.prepare_promise_all_resolve_element(realm, invocation, arguments)?
                .finish(self, realm)
        })
    }

    pub(crate) fn prepare_promise_all_resolve_element(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<operation::PromiseStep, RuntimeError> {
        let NativeInvocation::Call { .. } = invocation else {
            return Err(RuntimeError::Invariant(
                "Promise.all resolve-element received a constructor invocation",
            ));
        };
        let active = self.active_function()?;
        let internal = self
            .0
            .state
            .borrow()
            .heap
            .native_internal_callable(active.object_id())?
            .ok_or(RuntimeError::Invariant(
                "Promise.all resolve-element had no internal capture",
            ))?;
        let InternalCallableData::PromiseAllResolveElement {
            values,
            resolve,
            remaining,
            already_called,
            index,
        } = internal
        else {
            return Err(RuntimeError::Invariant(
                "Promise.all resolve-element had the wrong internal capture",
            ));
        };
        if already_called.replace(true) {
            return Ok(operation::PromiseStep::Complete(Completion::Return(
                JsValue::Undefined,
            )));
        }

        let value = self.promise_aggregate_element_argument(arguments)?;
        self.finish_promise_aggregate_element(
            realm,
            values,
            resolve,
            remaining,
            index,
            value,
            AggregateTerminal::ResolveValues,
        )
    }

    pub(crate) fn call_promise_all_settled_element(
        &self,
        target_outcome: PromiseReactionKind,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            self.prepare_promise_all_settled_element(target_outcome, realm, invocation, arguments)?
                .finish(self, realm)
        })
    }

    pub(crate) fn prepare_promise_all_settled_element(
        &self,
        target_outcome: PromiseReactionKind,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<operation::PromiseStep, RuntimeError> {
        let NativeInvocation::Call { .. } = invocation else {
            return Err(RuntimeError::Invariant(
                "Promise.allSettled element received a constructor invocation",
            ));
        };
        let active = self.active_function()?;
        let internal = self
            .0
            .state
            .borrow()
            .heap
            .native_internal_callable(active.object_id())?
            .ok_or(RuntimeError::Invariant(
                "Promise.allSettled element had no internal capture",
            ))?;
        let InternalCallableData::PromiseAllSettledElement {
            values,
            resolve,
            remaining,
            already_called,
            index,
            outcome,
        } = internal
        else {
            return Err(RuntimeError::Invariant(
                "Promise.allSettled element had the wrong internal capture",
            ));
        };
        if outcome != target_outcome {
            return Err(RuntimeError::Invariant(
                "Promise.allSettled element selector did not match its capture",
            ));
        }
        if already_called.replace(true) {
            return Ok(operation::PromiseStep::Complete(Completion::Return(
                JsValue::Undefined,
            )));
        }

        let value = self.promise_aggregate_element_argument(arguments)?;
        let result = self.new_ordinary_object_in_realm(realm)?;
        let (status, payload_name) = match outcome {
            PromiseReactionKind::Fulfill => ("fulfilled", "value"),
            PromiseReactionKind::Reject => ("rejected", "reason"),
        };
        self.define_fresh_promise_property(
            &result,
            "status",
            self.into_jsvalue(Value::String(JsString::from_static(status)))?,
            "fresh Promise.allSettled result rejected a data property",
        )?;
        self.define_fresh_promise_property(
            &result,
            payload_name,
            self.dup_jsvalue(value)?,
            "fresh Promise.allSettled result rejected a data property",
        )?;

        self.finish_promise_aggregate_element(
            realm,
            values,
            resolve,
            remaining,
            index,
            &JsValue::Object(result.object_id()),
            AggregateTerminal::ResolveValues,
        )
    }

    pub(crate) fn call_promise_any_reject_element(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            self.prepare_promise_any_reject_element(realm, invocation, arguments)?
                .finish(self, realm)
        })
    }

    pub(crate) fn prepare_promise_any_reject_element(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<operation::PromiseStep, RuntimeError> {
        let NativeInvocation::Call { .. } = invocation else {
            return Err(RuntimeError::Invariant(
                "Promise.any reject-element received a constructor invocation",
            ));
        };
        let active = self.active_function()?;
        let internal = self
            .0
            .state
            .borrow()
            .heap
            .native_internal_callable(active.object_id())?
            .ok_or(RuntimeError::Invariant(
                "Promise.any reject-element had no internal capture",
            ))?;
        let InternalCallableData::PromiseAnyRejectElement {
            errors,
            reject,
            remaining,
            already_called,
            index,
        } = internal
        else {
            return Err(RuntimeError::Invariant(
                "Promise.any reject-element had the wrong internal capture",
            ));
        };
        if already_called.replace(true) {
            return Ok(operation::PromiseStep::Complete(Completion::Return(
                JsValue::Undefined,
            )));
        }

        let reason = self.promise_aggregate_element_argument(arguments)?;
        self.finish_promise_aggregate_element(
            realm,
            errors,
            reject,
            remaining,
            index,
            reason,
            AggregateTerminal::RejectAggregate,
        )
    }

    fn promise_aggregate_element_argument<'a>(
        &self,
        arguments: &'a NativeArguments,
    ) -> Result<&'a JsValue, RuntimeError> {
        arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Promise aggregate element argv was not padded",
        ))
    }

    pub(super) fn define_fresh_promise_property(
        &self,
        object: &ObjectRef,
        name: &str,
        value: JsValue,
        rejection: &'static str,
    ) -> Result<(), RuntimeError> {
        let mut descriptor = OwnedPropertyDescriptor::new(self);
        descriptor.value = DescriptorField::Present(value);
        descriptor.writable = DescriptorField::Present(true);
        descriptor.enumerable = DescriptorField::Present(true);
        descriptor.configurable = DescriptorField::Present(true);
        let key = self.intern_property_key(name)?;
        match self.define_owned_property_in_realm(None, object, &key, &descriptor)? {
            PropertyDefineOutcome::Defined(true) => Ok(()),
            PropertyDefineOutcome::Defined(false) => Err(RuntimeError::Invariant(rejection)),
            PropertyDefineOutcome::Throw(value) => {
                self.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "context-free property definition produced a JavaScript throw",
                ))
            }
        }
    }

    // JS_PROP_THROW is deliberately absent: frozen aggregate arrays ignore false.
    fn define_promise_array_element(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        index: u32,
        value: &JsValue,
    ) -> Result<Option<JsValue>, RuntimeError> {
        let key = self.property_key_for_index(u64::from(index))?;
        let mut descriptor = OwnedPropertyDescriptor::new(self);
        descriptor.value = DescriptorField::Present(self.dup_jsvalue(value)?);
        descriptor.writable = DescriptorField::Present(true);
        descriptor.enumerable = DescriptorField::Present(true);
        descriptor.configurable = DescriptorField::Present(true);
        match self.internal_define_owned_property(realm, object, &key, descriptor)? {
            NativeConversion::Value(_) => Ok(None),
            NativeConversion::Throw(value) => Ok(Some(value)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_promise_aggregate_element(
        &self,
        realm: ContextId,
        values: ObjectId,
        settle: ObjectId,
        remaining: Rc<Cell<i32>>,
        index: u32,
        value: &JsValue,
        terminal: AggregateTerminal,
    ) -> Result<operation::PromiseStep, RuntimeError> {
        let values = ObjectRef::from_borrowed_handle(self.clone(), values)?;
        if let Some(value) = self.define_promise_array_element(realm, &values, index, value)? {
            return Ok(operation::PromiseStep::Complete(Completion::Throw(value)));
        }

        let count = remaining
            .get()
            .checked_sub(1)
            .ok_or(RuntimeError::Invariant(
                "Promise aggregate element counter underflowed",
            ))?;
        remaining.set(count);
        if count != 0 {
            return Ok(operation::PromiseStep::Complete(Completion::Return(
                JsValue::Undefined,
            )));
        }

        let argument = match terminal {
            AggregateTerminal::ResolveValues => values.clone(),
            AggregateTerminal::RejectAggregate => {
                self.new_internal_aggregate_error(realm, values.clone())?
            }
        };
        let settle = ObjectRef::from_borrowed_handle(self.clone(), settle)?;
        let settle = self.as_callable(&settle)?.ok_or(RuntimeError::Invariant(
            "Promise aggregate final settlement function was no longer callable",
        ))?;
        Ok(operation::PromiseStep::ignore_return(
            realm,
            settle,
            JsValue::Object(argument.into_handle()),
        ))
    }
}
