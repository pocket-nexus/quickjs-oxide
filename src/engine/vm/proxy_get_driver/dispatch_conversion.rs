//! Bounded native-stack dispatch for conversion requests.
use super::{
    Error, Next, Query, Resume, ReturnOwner, RunningExecution, Runtime, Step,
    runtime_error_to_vm_error,
};

#[inline(never)]
pub(super) fn primitive(
    runtime: &Runtime,
    _execution: &mut RunningExecution,
    _owner: ReturnOwner,
    _identity: u64,
    query: &mut Query,
    pending: &mut Step,
) -> Result<Next, Error> {
    let step = pending;
    loop {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "dispatch_conversion.primitive.visit",
        );
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("conversion_transition");
        let realm = query.realm;
        match &mut *step {
            Step::String { value, resume } => {
                let value = value.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");

                *step = Step::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::String),
                    resume: Some(Resume::StringValue {
                        realm,
                        resume: Box::new(resume),
                    }),
                };
                continue;
            }
            Step::OrdinaryPrimitive { object, hint } => {
                let object = object.take().expect("selected Step field");
                let hint = hint.take().expect("selected Step field");

                *step = crate::engine::value::conversion::primitive::PrimitiveResume::ordinary(
                    runtime, realm, object, hint,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::Arguments { value, resume } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("argument continuation allocation failed"))?;
                let value = value.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");

                query.parents.push(resume);
                *step = crate::engine::builtins::ArgumentsStep::start(runtime, realm, value)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::ArgumentsComplete(result) => {
                let Some(parent) = query.parents.pop() else {
                    return Err(Error::internal("argument list lost its continuation"));
                };
                let result = result.take().expect("selected Step field");
                *step = parent
                    .arguments(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::Primitive {
                value,
                hint,
                resume,
            } => {
                let value = value.take().expect("selected Step field");
                let hint = hint.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");

                let next = crate::engine::value::conversion::primitive::PrimitiveResume::start(
                    runtime, realm, value, hint,
                );
                *step = match next {
                    crate::engine::value::conversion::primitive::PrimitiveStep::Complete(
                        result,
                    ) => resume
                        .resume(runtime, result)
                        .map_err(runtime_error_to_vm_error)?,
                    next => {
                        if query.parents.try_reserve(1).is_err() {
                            let mut abandoned: Step = next.into();
                            abandoned.release_owned(runtime);
                            resume.release_owned();
                            return Err(Error::internal(
                                "primitive continuation allocation failed",
                            ));
                        }
                        query.parents.push(resume);
                        next.into()
                    }
                };
                continue;
            }
            Step::Number { value, resume } => {
                let value = value.take().expect("selected Step field");

                // Only a request which can suspend needs a parent owner. Complete
                // results (including JS throws) use the same typed reply consumer.
                let next = crate::engine::value::conversion::number::NumberStep::start_jsvalue(
                    runtime, realm, value,
                )
                .map_err(runtime_error_to_vm_error)?;
                let resume = resume.take().expect("selected Step field");
                *step = match next {
                    crate::engine::value::conversion::number::NumberStep::Complete(result) => {
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_owned_execution_event(
                            "conversion_immediate",
                        );
                        resume
                            .number(runtime, result)
                            .map_err(runtime_error_to_vm_error)?
                    }
                    next => {
                        if query.parents.try_reserve(1).is_err() {
                            let mut abandoned: Step = next.into();
                            abandoned.release_owned(runtime);
                            resume.release_owned();
                            return Err(Error::internal("property continuation allocation failed"));
                        }
                        query.parents.push(resume);
                        next.into()
                    }
                };
                continue;
            }
            Step::NumberComplete(result) => {
                let Some(resume) = query.parents.pop() else {
                    return Err(Error::internal(
                        "ToNumber result has no matching continuation",
                    ));
                };
                let result = result.take().expect("selected Step field");
                *step = resume
                    .number(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::LengthComplete(result) => {
                let resume = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("Array length result has no parent"))?;
                let result = result.take().expect("selected Step field");
                *step = resume
                    .length(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::Element {
                element,
                value,
                resume,
            } => {
                let element = element.take().expect("selected Step field");
                let value = value.take().expect("selected Step field");

                // Only a request which can suspend needs a parent owner. Complete
                // results (including JS throws) use the same typed reply consumer.
                let next =
                    crate::engine::builtins::ElementStep::start(runtime, realm, element, value)
                        .map_err(runtime_error_to_vm_error)?;
                let resume = resume.take().expect("selected Step field");
                *step = match next {
                    crate::engine::builtins::ElementStep::Complete(result) => resume
                        .element(runtime, result)
                        .map_err(runtime_error_to_vm_error)?,
                    next => {
                        if query.parents.try_reserve(1).is_err() {
                            let mut abandoned: Step = next.into();
                            abandoned.release_owned(runtime);
                            resume.release_owned();
                            return Err(Error::internal("property continuation allocation failed"));
                        }
                        query.parents.push(resume);
                        next.into()
                    }
                };
                continue;
            }
            Step::ElementComplete(result) => {
                let Some(resume) = query.parents.pop() else {
                    return Err(Error::internal(
                        "element result has no matching continuation",
                    ));
                };
                let result = result.take().expect("selected Step field");
                *step = resume
                    .element(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::TypedComplete(result) => {
                let resume = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("TypedArray result has no parent"))?;
                let result = result.take().expect("selected Step field");
                *step = resume
                    .typed(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            _ => {
                return Ok(Next::Continue);
            }
        }
    }
}

#[inline(never)]
pub(super) fn constructor(
    runtime: &Runtime,
    _execution: &mut RunningExecution,
    _owner: ReturnOwner,
    _identity: u64,
    query: &mut Query,
    pending: &mut Step,
) -> Result<Next, Error> {
    let step = pending;
    loop {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "dispatch_conversion.constructor.visit",
        );
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("conversion_transition");
        let realm = query.realm;
        match &mut *step {
            Step::ConstructorSource { new_target, resume } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("constructor source continuation allocation failed")
                })?;
                let new_target = new_target.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");
                query.parents.push(resume);
                *step = super::super::call::prototype::ProtoSourceStep::start(
                    runtime, realm, new_target,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::ConstructorSourceComplete(result) => {
                let parent = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("constructor source has no parent"))?;
                let result = result.take().expect("selected Step field");
                *step = parent
                    .constructor_source(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::TypedSpeciesView {
                source,
                element,
                buffer,
                offset,
                length,
                resume,
            } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("typed species view continuation allocation failed")
                })?;
                let source = source.take().expect("selected Step field");
                let element = element.take().expect("selected Step field");
                let buffer = buffer.take().expect("selected Step field");
                let offset = offset.take().expect("selected Step field");
                let length = length.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");

                query.parents.push(resume);
                *step = crate::engine::builtins::TypedSpeciesStep::start_view(
                    runtime, realm, source, element, buffer, offset, length,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::TypedIteratorMethod { source, resume } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("typed iterator method continuation allocation failed")
                })?;
                let source = source.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");

                query.parents.push(resume);
                *step =
                    crate::engine::builtins::TypedIteratorMethodStep::start(runtime, realm, source)
                        .map_err(runtime_error_to_vm_error)?
                        .into();
                continue;
            }
            Step::TypedIteratorMethodComplete(result) => {
                let resume = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("typed iterator method lost parent"))?;
                let result = result.take().expect("selected Step field");
                *step = resume
                    .typed_iterator_method(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::TypedCollect {
                source,
                method,
                element,
                resume,
            } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("typed collection continuation allocation failed")
                })?;
                let source = source.take().expect("selected Step field");
                let method = method.take().expect("selected Step field");
                let element = element.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");

                query.parents.push(resume);
                *step = crate::engine::builtins::TypedCollectStep::start(
                    runtime, realm, source, method, element,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::TypedCollectComplete(result) => {
                let Some(resume) = query.parents.pop() else {
                    return Err(Error::internal("typed collection lost parent"));
                };
                let result = result.take().expect("selected Step field");
                *step = resume
                    .typed_collected(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::TypedCreate {
                constructor,
                length,
                resume,
            } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("typed creation continuation allocation failed")
                })?;
                let constructor = constructor.take().expect("selected Step field");
                let length = length.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");

                query.parents.push(resume);
                *step = crate::engine::builtins::TypedSpeciesStep::create(
                    runtime,
                    realm,
                    constructor,
                    vec![
                        crate::engine::value::number::operations::Number::compact(length as f64)
                            .into(),
                    ],
                    Some(length),
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::TypedSpecies {
                source,
                element,
                length,
                resume,
            } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("TypedArray species continuation allocation failed")
                })?;
                let source = source.take().expect("selected Step field");
                let element = element.take().expect("selected Step field");
                let length = length.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");

                query.parents.push(resume);
                *step = crate::engine::builtins::TypedSpeciesStep::start(
                    runtime, realm, source, element, length,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::TypedSpeciesComplete(result) => {
                let parent = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("TypedArray species has no parent"))?;
                let result = result.take().expect("selected Step field");
                *step = parent
                    .typed_species(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            _ => {
                return Ok(Next::Continue);
            }
        }
    }
}
