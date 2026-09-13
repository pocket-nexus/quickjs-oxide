//! One get_proxy_method protocol for all Proxy operations.
//! Method lookup owns its depth guard until the selected method is delivered.
use super::{ProxyMethodStackGuard, RootedProxy};
use crate::engine::api::{
    error::{ErrorKind, NativeErrorKind},
    runtime::Runtime,
    runtime_error::RuntimeError,
};
use crate::engine::heap::ContextId;
use crate::engine::object::{ObjectRef, PropertyKey};
use crate::engine::value::{Value, conversion::NativeConversion};
use crate::engine::vm::{Completion, call::DirectCallTarget};

pub(super) enum MethodStep {
    Complete(NativeConversion<(RootedProxy, Option<DirectCallTarget>)>),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: MethodResume,
    },
}

pub(super) struct MethodResume {
    rooted: RootedProxy,
    search: Search,
}

struct Search {
    realm: ContextId,
    key: PropertyKey,
    limit: Option<usize>,
    depth: usize,
    _guard: ProxyMethodStackGuard,
}

impl MethodStep {
    pub(super) fn start(
        runtime: &Runtime,
        realm: ContextId,
        proxy: ObjectRef,
        name: &'static str,
    ) -> Result<Self, RuntimeError> {
        if runtime.proxy_method_stack_would_overflow() {
            return overflow(runtime, realm);
        }
        let guard = ProxyMethodStackGuard::enter(runtime);
        let key = runtime.intern_property_key(name)?;
        Search {
            realm,
            key,
            limit: runtime.proxy_method_chain_limit(name),
            depth: 0,
            _guard: guard,
        }
        .read(runtime, proxy)
    }
}

fn overflow(runtime: &Runtime, realm: ContextId) -> Result<MethodStep, RuntimeError> {
    Ok(MethodStep::Complete(NativeConversion::Throw(
        runtime.new_native_error(realm, NativeErrorKind::Internal, "stack overflow")?,
    )))
}

impl Search {
    fn read(self, runtime: &Runtime, proxy: ObjectRef) -> Result<MethodStep, RuntimeError> {
        if self.limit.is_some_and(|limit| self.depth == limit) {
            return overflow(runtime, self.realm);
        }
        let data = runtime
            .proxy_snapshot_if_any(&proxy)?
            .ok_or(RuntimeError::Invariant(
                "Proxy method dispatch reached an ordinary object",
            ))?;
        if data.is_revoked {
            return Ok(MethodStep::Complete(
                runtime.proxy_revoked_throw(self.realm)?,
            ));
        }
        let rooted = runtime.root_proxy_snapshot(&proxy, data)?;
        Ok(MethodStep::Read {
            object: rooted.handler.clone(),
            key: self.key.clone(),
            receiver: Value::Object(rooted.handler.clone()),
            resume: MethodResume {
                rooted,
                search: self,
            },
        })
    }
}

impl MethodResume {
    pub(super) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<MethodStep, RuntimeError> {
        let Self { rooted, mut search } = self;
        let value = match completion {
            Completion::Throw(value) => {
                return Ok(MethodStep::Complete(NativeConversion::Throw(value)));
            }
            Completion::Return(value) => value,
        };
        if matches!(value, Value::Undefined | Value::Null) {
            if runtime.proxy_snapshot_if_any(&rooted.target)?.is_some() {
                search.depth = search.depth.saturating_add(1);
                return search.read(runtime, rooted.target);
            }
            return Ok(MethodStep::Complete(NativeConversion::Value((
                rooted, None,
            ))));
        }
        let method = match runtime.direct_call_target_from_value(value) {
            Ok(method) => method,
            Err(RuntimeError::Engine(error)) if error.kind() == ErrorKind::Type => {
                return Ok(MethodStep::Complete(NativeConversion::Throw(
                    runtime.new_native_error_from_error(
                        search.realm,
                        NativeErrorKind::Type,
                        &error,
                    )?,
                )));
            }
            Err(error) => return Err(error),
        };
        Ok(MethodStep::Complete(NativeConversion::Value((
            rooted,
            Some(method),
        ))))
    }
}
