//! Mechanical adapters for buffer domain requests.
use super::JsValue;
use super::{DirectCallTarget, ElementStep, Resume, Step, TypedWriteStep};

step_from! {
    ElementStep => Element;
    Read => read(take_read_object, take_read_key);
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    raw {
        T::Complete(result) => Self::ElementComplete(Some(result)),
    }
}

step_from! {
    TypedWriteStep => TypedElement;
    Element { element, value } => Element;
    raw {
        T::Complete(result) => Self::TypedComplete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::DataViewAccessStep => DataView;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::Number };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::BufferMutationStep => BufferMutation;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::Number };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::TypedTraversalStep => TypedTraversal;
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::TypedSpeciesStep => TypedSpecies;
    Read { object, key } => read;
    raw {
        T::Complete(result) => Self::TypedSpeciesComplete(Some(result)),
        T::Construct {
            constructor,
            mut resume,
        } => Self::Construct {
            new_target: Some(crate::engine::vm::call::ConstructNewTarget::Validated(
                constructor.try_clone()?,
            )),
            target: Some(constructor),
            arguments: Some(resume.take_arguments()),
            resume: Some(Resume::TypedSpecies(resume)),
        },
    }
}

step_from! {
    crate::engine::builtins::TypedIterationStep => TypedIteration;
    Read => read(take_read_object, take_read_key);
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Species => TypedSpecies { source: take_species_source, element: take_species_element,
        length: take_species_length };
    Element => Element { element: take_element_element, value: take_element_value };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::TypedSortStep => TypedSort;
    Number { value } => Number;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Call {
            callable,
            arguments,
            resume,
        } => Self::Call {
            target: Some(DirectCallTarget::Callable(callable)),
            receiver: Some(JsValue::Undefined),
            arguments: Some(arguments),
            resume: Some(Resume::TypedSort(resume)),
        },
    }
}

step_from! {
    crate::engine::builtins::BufferConstructorStep => BufferConstructor;
    Read { object, key } => read;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::Number };
    Prototype { new_target } => ConstructorSource;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::DataViewConstructorStep => DataViewConstructor;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::Number };
    Prototype { new_target } => ConstructorSource;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::TypedSetStep => TypedSet;
    Read { object, key } => read;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::Number };
    Element { element, value } => Element;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::TypedSearchStep => TypedSearch;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::Number };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::TypedStringStep => TypedString;
    Primitive => Primitive { value: take_primitive_value;
        hint: crate::engine::vm::ToPrimitiveHint::String };
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::TypedSliceStep => TypedSlice;
    Primitive => Primitive { value: take_primitive_value;
        hint: crate::engine::vm::ToPrimitiveHint::Number };
    Species => TypedSpecies { source: take_species_source, element: take_species_element,
        length: take_species_length };
    SpeciesView => TypedSpeciesView { source: take_species_view_source,
        element: take_species_view_element, buffer: take_species_view_buffer,
        offset: take_species_view_offset, length: take_species_view_length };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::TypedMutationStep => TypedMutation;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::Number };
    Element { element, value } => Element;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::BufferSliceStep => BufferSlice;
    Read { object, key } => read;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::Number };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Construct {
            constructor,
            arguments,
            resume,
        } => Self::Construct {
            new_target: Some(crate::engine::vm::call::ConstructNewTarget::Validated(
                constructor.try_clone()?,
            )),
            target: Some(constructor),
            arguments: Some(arguments),
            resume: Some(Resume::BufferSlice(resume)),
        },
    }
}

step_from! {
    crate::engine::builtins::TypedWithStep => TypedWith;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::Number };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::Uint8CodecStep => Uint8Codec;
    Read { object, key } => read;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::TypedIteratorMethodStep => TypedIteratorMethod;
    raw {
        T::Complete(result) => Self::TypedIteratorMethodComplete(Some(result)),
        T::Read { key, mut resume } => Self::ReadValue {
            receiver: Some(resume.take_receiver()),
            key: Some(key),
            resume: Some(Resume::TypedIteratorMethod(resume)),
        },
    }
}

step_from! {
    crate::engine::builtins::TypedCollectStep => TypedCollect;
    Read { object, key } => read;
    raw {
        T::Complete(result) => Self::TypedCollectComplete(Some(result)),
        T::Call {
            callable,
            mut resume,
        } => Self::Call {
            target: Some(DirectCallTarget::Callable(callable)),
            receiver: Some(resume.take_receiver()),
            arguments: Some(Vec::new()),
            resume: Some(Resume::TypedCollect(resume)),
        },
    }
}

step_from! {
    crate::engine::builtins::TypedCreateStep => TypedCreate;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Primitive => Primitive { value: take_primitive_value;
        hint: crate::engine::vm::ToPrimitiveHint::Number };
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Prototype => ConstructorSource { new_target: take_prototype_new_target };
    Method => TypedIteratorMethod { source: take_method_source };
    Collect => TypedCollect { source: take_collect_source, method: take_collect_method,
        element: take_collect_element };
    Create => TypedCreate { constructor: take_create_constructor, length: take_create_length };
    Element => Element { element: take_element_element, value: take_element_value };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::AtomicsStep => Atomics;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::Number };
    Number { value } => Number;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}
