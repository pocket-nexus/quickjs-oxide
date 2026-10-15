//! Mechanical adapters for array domain requests.
use super::{DirectCallTarget, JsValue, Resume, Step};

step_from! {
    crate::engine::builtins::ArrayMutationStep => ArrayMutation;
    PreparedRead => PreparedRead { read: take_prepared_read_read, key: take_prepared_read_key };
    Read => read(take_read_object, take_read_key);
    Number => Number { value: take_number_value };
    Copy => ArrayCopy { object: take_copy_object, to: take_copy_to, from: take_copy_from,
        count: take_copy_count, backwards: take_copy_backwards };
    Set => set_keyed(take_set_object, take_set_key, take_set_value) @ ArrayMutationSet;
    Delete => Delete { object: take_delete_object, key: take_delete_key };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::PreparedSet { mut resume } => {
            let step = resume.take_prepared_set_step();
            let key = resume.take_prepared_set_key();
            Self::PreparedSet {
                step: Some(step),
                resume: Some(Resume::ArrayMutationSet {
                    resume: resume.with_scheduler_set_key(key),
                }),
            }
        }
    }
}

step_from! {
    crate::engine::builtins::ArrayCallbackStep => ArrayCallback;
    Read => read(take_read_object, take_read_key);
    Number => Number { value: take_number_value };
    Has => Has { object: take_has_object, key: take_has_key };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Species => ArraySpecies { source: take_species_source, length: take_species_length };
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ArraySpeciesStep => ArraySpecies;
    Read { object, key } => read;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Construct { target, arguments } => {
            let new_target = match target.try_clone() {
                Ok(target) => target,
                Err(error) => {
                    for value in arguments {
                        let _ = target.as_object().runtime().release_jsvalue(value);
                    }
                    return Err(error);
                }
            };
            Self::Construct {
                new_target: Some(crate::engine::vm::call::ConstructNewTarget::Validated(
                    new_target,
                )),
                target: Some(target),
                arguments: Some(arguments),
                resume: Some(Resume::Identity),
            }
        }
    }
}

step_from! {
    crate::engine::builtins::ArrayNextStep => ArrayNext;
    PreparedRead => PreparedRead { read: take_prepared, key: take_key };
    Read => read(take_read);
    Number => Number { value: take_number };
    raw {
        T::Complete(result) => Self::NativeRawComplete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ArraySortStep => ArraySort;
    Read => read(take_read_object, take_read_key);
    Number => Number { value: take_number_value };
    Has => Has { object: take_has_object, key: take_has_key };
    Set => set_keyed(take_set_object, take_set_key, take_set_value) @ ArraySortSet;
    Delete => Delete { object: take_delete_object, key: take_delete_key };
    String => String { value: take_string_value };
    Call => Call { target: take_call_callable as DirectCallTarget::Callable,
        arguments: take_call_arguments; receiver: JsValue::Undefined };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ArrayIndexedStep => ArrayIndexed;
    Read => read(take_read_object, take_read_key);
    Number => Number { value: take_number_value };
    Has => Has { object: take_has_object, key: take_has_key };
    Set => set_keyed(take_set_object, take_set_key, take_set_value) @ ArrayIndexedSet;
    Copy => ArrayCopy { object: take_copy_object, to: take_copy_to, from: take_copy_from,
        count: take_copy_count, backwards: take_copy_backwards };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ArrayReverseStep => ArrayReverse;
    Read => read(take_read_object, take_read_key);
    Number => Number { value: take_number_value };
    Has => Has { object: take_has_object, key: take_has_key };
    Set => set_keyed(take_set_object, take_set_key, take_set_value) @ ArrayReverseSet;
    Delete => Delete { object: take_delete_object, key: take_delete_key };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ArrayStringStep => ArrayString;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Number => Number { value: take_number_value };
    String => String { value: take_string_value };
    Call => call(take_call_callable, take_call_receiver);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::ObjectTag { receiver } => Self::ObjectTag {
            receiver: Some(receiver),
        },
    }
}

step_from! {
    crate::engine::builtins::ArrayBuildStep => ArrayBuild;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Number => Number { value: take_number_value };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Set => set_keyed(take_set_object, take_set_key, take_set_value) @ ArrayBuildSet;
    Construct => construct(take_construct_target, take_construct_arguments);
    Parse => ParseIterator { result: take_parse_result };
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Close {
            iterator,
            completion,
        } => Self::IteratorClose {
            iterator: Some(iterator),
            completion: Some(completion),
        },
    }
}

step_from! {
    crate::engine::builtins::ArrayCopyStep => ArrayCopy;
    Read => read(take_read_object, take_read_key);
    Has => Has { object: take_has_object, key: take_has_key };
    Set => set_keyed(take_set_object, take_set_key, take_set_value) @ ArrayCopySet;
    Delete => Delete { object: take_delete_object, key: take_delete_key };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ArrayConcatStep => ArrayConcat;
    Read => read(take_read_object, take_read_key);
    Number => Number { value: take_number_value };
    Has => Has { object: take_has_object, key: take_has_key };
    Set => set_keyed(take_set_object, take_set_key, take_set_value) @ ArrayConcatSet;
    Species => ArraySpecies { source: take_species_source; length: 0 };
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ArrayFlattenStep => ArrayFlatten;
    Read => read(take_read_object, take_read_key);
    Number => Number { value: take_number_value };
    Has => Has { object: take_has_object, key: take_has_key };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Species => ArraySpecies { source: take_species_source; length: 0 };
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ArrayConstructorStep => ArrayConstructor;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Set => set_keyed(take_set_object, take_set_key, take_set_value) @ ArrayConstructorSet;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ArraySliceStep => ArraySlice;
    Read => read(take_read);
    raw {
        T::Return(value) => {
            Self::Complete(Some(crate::engine::vm::Completion::Return(value)))
        }
        T::Throw(value) => {
            Self::Complete(Some(crate::engine::vm::Completion::Throw(value)))
        }
        T::PreparedRead { mut resume } => {
            let (read, key) = resume.take_preparedread();
            Self::PreparedRead {
                read: Some(read),
                key: Some(key),
                resume: Some(Resume::ArraySlice(resume)),
            }
        }
        T::PreparedHas { mut resume } => {
            let (probe, key) = resume.take_preparedhas();
            Self::PreparedHas {
                probe: Some(probe),
                key: Some(key),
                resume: Some(Resume::ArraySlice(resume)),
            }
        }
        T::Number { mut resume } => {
            let (value,) = resume.take_number();
            Self::Number {
                value: Some(value),
                resume: Some(Resume::ArraySlice(resume)),
            }
        }
        T::Set { mut resume } => {
            let (object, key, value) = resume.take_set();
            let receiver = match object.try_clone() {
                Ok(receiver) => receiver,
                Err(error) => {
                    let _ = object.runtime().release_jsvalue(value);
                    return Err(error);
                }
            };
            let scheduler_key = match key.try_clone() {
                Ok(key) => key,
                Err(error) => {
                    let _ = object.runtime().release_jsvalue(value);
                    return Err(error);
                }
            };
            Self::Set {
                receiver: Some(JsValue::Object(receiver.into_handle())),
                object: Some(object),
                key: Some(scheduler_key),
                value: Some(value),
                resume: Some(Resume::ArraySliceSet {
                    resume: resume.with_scheduler_set_key(key),
                }),
            }
        }
        T::Delete { mut resume } => {
            let (object, key) = resume.take_delete();
            Self::Delete {
                object: Some(object),
                key: Some(key),
                resume: Some(Resume::ArraySlice(resume)),
            }
        }
        T::Species { mut resume } => {
            let (source, length) = resume.take_species();
            Self::ArraySpecies {
                source: Some(source),
                length: Some(length),
                resume: Some(Resume::ArraySlice(resume)),
            }
        }
        T::Define { mut resume } => {
            let (object, key, descriptor) = resume.take_define();
            Self::Define {
                object: Some(object),
                key: Some(key),
                descriptor: Some(descriptor.into()),
                resume: Some(Resume::ArraySlice(resume)),
            }
        }
        T::Copy { mut resume } => {
            let (object, to, from, count, backwards) = resume.take_copy();
            Self::ArrayCopy {
                object: Some(object),
                to: Some(to),
                from: Some(from),
                count: Some(count),
                backwards: Some(backwards),
                resume: Some(Resume::ArraySlice(resume)),
            }
        }
    }
}
