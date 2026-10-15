//! Mechanical adapters for object builtins domain requests.
use super::{JsValue, Resume, Step};

step_from! {
    crate::engine::builtins::BuiltinPrototypeStep => BuiltinPrototype;
    Get { object } => GetPrototype;
    Set { object, prototype } => SetPrototype;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::PropertyStep => Property;
    Keys => Keys { object: take_keys_object };
    Key => Primitive { value: take_key_value;
        hint: crate::engine::vm::ToPrimitiveHint::String } @ PropertyKey;
    Convert => Convert { value: take_convert_value };
    Read => Read { object: take_read_object, key: take_read_key, receiver: take_read_receiver };
    Set => Set { object: take_set_object, key: take_set_key, value: take_set_value,
        receiver: take_set_receiver };
    Has => Has { object: take_has_object, key: take_has_key };
    Delete => Delete { object: take_delete_object, key: take_delete_key };
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    Descriptor => Descriptor { object: take_descriptor_object, key: take_descriptor_key };
    Extensible => Extensible { object: take_extensible_object };
    Prevent => PreventExtensions { object: take_prevent_object };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::PredicateStep => Predicate;
    Descriptor => Descriptor { object: take_descriptor_object, key: take_descriptor_key };
    Prototype => GetPrototype { object: take_prototype_object };
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    Key => Primitive { value: take_key_value;
        hint: crate::engine::vm::ToPrimitiveHint::String } @ PredicateKey;
    Own => OwnFlag { object: take_own_object, key: take_own_key, enumerable: take_own_enumerable };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::DefinitionsStep => Definitions;
    Keys => Keys { object: take_keys_object };
    Enumerable => SnapshotEnumerable { object: take_enumerable_object, key: take_enumerable_key };
    Read => read(take_read_object, take_read_key);
    Convert => Convert { value: take_convert_value };
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ObjectStringStep => ObjectString;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Call => Call { target: take_call_target, receiver: take_call_receiver; arguments: Vec::new() };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ObjectIterationStep => ObjectIteration;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Next => IteratorNext { iterator: take_next_iterator, method: take_next_method };
    Key => Primitive { value: take_key_value;
        hint: crate::engine::vm::ToPrimitiveHint::String } @ ObjectIterationKey;
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    Push => ArrayPush { object: take_push_object, value: take_push_value };
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
    crate::engine::builtins::ObjectCopyStep => ObjectCopy;
    Read { object, key } => read;
    Keys { object } => Keys;
    Enumerable { object, key } => SnapshotEnumerable;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),

        T::PreparedRead(prepared) => {
            let (read, key, resume) = prepared.into_parts();
            Self::PreparedRead {
                read: Some(read),
                key: Some(key),
                resume: Some(Resume::ObjectCopy(resume)),
            }
        }
    }
}

step_from! {
    crate::engine::builtins::ObjectConstructorStep => ObjectConstructor;
    Prototype { new_target } => ConstructorSource;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::JsonParseStep => JsonParse;
    Read => read(take_read_object, take_read_key);
    String => String { value: take_string_value };
    Number => Number { value: take_number_value };
    Keys => Keys { object: take_keys_object };
    Enumerable => SnapshotEnumerable { object: take_enumerable_object, key: take_enumerable_key };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Delete => Delete { object: take_delete_object, key: take_delete_key };
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::JsonStringifyStep => JsonStringify;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    String => String { value: take_string_value };
    Number => Number { value: take_number_value };
    Keys => Keys { object: take_keys_object };
    Enumerable => OwnFlag { object: take_enumerable_object, key: take_enumerable_key;
        enumerable: true };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}
