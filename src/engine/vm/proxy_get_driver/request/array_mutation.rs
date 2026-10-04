//! Selected Array effects leave State only for existing Copy/Proxy/shared work.
use super::{Resume, Runtime, Step};
use crate::engine::{
    api::RuntimeError,
    builtins::ArrayMutationStep,
    object::{ObjectRef, PropertyKey, ReadStep},
};

pub(in crate::engine::vm::proxy_get_driver) fn consume_boundary(
    runtime: &Runtime,
    pending: &mut Step,
) -> Result<(), RuntimeError> {
    match pending {
        Step::ArrayMutationRead { read, resume } => {
            if matches!(read, Some(ReadStep::Shared(_))) {
                let Some(ReadStep::Shared(word)) = read.take() else {
                    unreachable!()
                };
                let word = word.read()?;
                *read = Some(ReadStep::Ready(
                    runtime.0.state.borrow_mut().own_typed_read_word(word)?,
                ));
            } else {
                // The original dynamic key role transfers to the actual Proxy
                // request. No independent checked promotion is introduced.
                let atom = resume.as_mut().expect("Array read parent").take_read_key();
                let key = PropertyKey::from_owned_atom(runtime.clone(), atom);
                let selected = match read.take().expect("selected Array Proxy") {
                    ReadStep::Ready(read) | ReadStep::CyclePublished(read) => read,
                    ReadStep::Shared(_) => unreachable!(),
                };
                *pending = Step::PreparedRead {
                    read: Some(runtime.adopt_prepared_read(selected)),
                    key: Some(key),
                    resume: Some(Resume::ArrayMutation(
                        resume.take().expect("Array read parent"),
                    )),
                };
            }
        }
        Step::ArrayMutationSharedDelete { word, resume } => {
            word.take().expect("selected shared delete word").read()?;
            let mut state = runtime.0.state.borrow_mut();
            *pending = Step::from(
                resume
                    .take()
                    .expect("shared delete parent")
                    .boolean_in_state(
                        &mut state,
                        &runtime.0.poisoned,
                        crate::engine::value::conversion::NativeConversion::Value(false),
                    )?,
            );
        }
        Step::ArrayMutationProgress(progress) => {
            *pending = match progress.take().expect("selected Array boundary") {
                ArrayMutationStep::Copy {
                    object,
                    to,
                    from,
                    count,
                    backwards,
                    resume,
                } => Step::ArrayCopy {
                    object: Some(ObjectRef::from_owned_handle(runtime.clone(), object)),
                    to: Some(to),
                    from: Some(from),
                    count: Some(count),
                    backwards: Some(backwards),
                    resume: Some(Resume::ArrayMutation(resume)),
                },
                ArrayMutationStep::Delete {
                    object,
                    key,
                    resume,
                } => Step::Delete {
                    object: Some(ObjectRef::from_owned_handle(runtime.clone(), object)),
                    key: Some(PropertyKey::from_owned_atom(runtime.clone(), key)),
                    resume: Some(Resume::ArrayMutation(resume)),
                },
                progress => {
                    progress.retire_at_boundary(runtime)?;
                    return Err(RuntimeError::Invariant(
                        "unselected Array effect reached boundary",
                    ));
                }
            };
        }
        _ => {
            return Err(RuntimeError::Invariant(
                "Array boundary lost selected effect",
            ));
        }
    }
    runtime.check_poison()
}
