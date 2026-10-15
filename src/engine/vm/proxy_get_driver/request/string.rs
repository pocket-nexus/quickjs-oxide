//! Mechanical adapters for string domain requests.
use super::{JsValue, Resume, Step};

step_from! {
    crate::engine::builtins::StringReplaceStep => StringReplace;
    PreparedRead => PreparedRead { read: take_preparedread_read, key: take_preparedread_key };
    Primitive => Primitive { value: take_primitive_value;
        hint: crate::engine::vm::ToPrimitiveHint::String };
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpExecStep => RegExpExec;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Primitive => Primitive { value: take_primitive_value, hint: take_primitive_hint };
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpPresentationStep => RegExpPresentation;
    Read { object, key } => read;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::String };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpReplaceStep => RegExpReplace;
    PreparedSet => PreparedSet { step: take_preparedset_step };
    PreparedRead => PreparedRead { read: take_preparedread_read, key: take_preparedread_key };
    Primitive => Primitive { value: take_primitive_value, hint: take_primitive_hint };
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Exec => RegExpExec { regexp: take_exec_regexp, input: take_exec_input };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::StringTextStep => StringText;
    Primitive { value, hint } => Primitive;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::StringSearchStep => StringSearch;
    Read { object, key } => read;
    Primitive { value, hint } => Primitive;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::StringSplitStep => StringSplit;
    Read => read(take_read_object, take_read_key);
    Primitive => Primitive { value: take_primitive_value, hint: take_primitive_hint };
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpConstructorStep => RegExpConstructor;
    Read { object, key } => read;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::String };
    Prototype { new_target } => ConstructorSource;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpCompileStep => RegExpCompile;
    Primitive { value } => Primitive { hint: crate::engine::vm::ToPrimitiveHint::String };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::StringProtocolStep => StringProtocol;
    Read => read(take_read_object, take_read_key);
    Primitive => Primitive { value: take_primitive_value;
        hint: crate::engine::vm::ToPrimitiveHint::String };
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Construct => construct(take_construct_constructor, take_construct_arguments);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpSearchStep => RegExpSearch;
    Read => read(take_read_object, take_read_key);
    Primitive => Primitive { value: take_primitive_value;
        hint: crate::engine::vm::ToPrimitiveHint::String };
    Exec => RegExpExec { regexp: take_exec_regexp, input: take_exec_input };
    Set => set(take_set_object, take_set_key, take_set_value);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpMatchStep => RegExpMatch;
    Read => read(take_read_object, take_read_key);
    Primitive => Primitive { value: take_primitive_value, hint: take_primitive_hint };
    Exec => RegExpExec { regexp: take_exec_regexp, input: take_exec_input };
    Set => set(take_set_object, take_set_key, take_set_value);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpMatchAllStep => RegExpMatchAll;
    Read => read(take_read_object, take_read_key);
    Primitive => Primitive { value: take_primitive_value, hint: take_primitive_hint };
    Species => RegExpSpecies { regexp: take_species_regexp };
    Construct => construct(take_construct_constructor, take_construct_arguments);
    Set => set(take_set_object, take_set_key, take_set_value);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpSplitStep => RegExpSplit;
    Read => read(take_read_object, take_read_key);
    Primitive => Primitive { value: take_primitive_value, hint: take_primitive_hint };
    Species => RegExpSpecies { regexp: take_species_regexp };
    Construct => construct(take_construct_constructor, take_construct_arguments);
    Set => set(take_set_object, take_set_key, take_set_value);
    Exec => RegExpExec { regexp: take_exec_regexp, input: take_exec_input };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpSpeciesStep => RegExpSpecies;
    Read { object, key } => read;
    raw {
        T::Complete(result) => Self::RegExpSpeciesComplete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::RegExpIteratorStep => RegExpIterator;
    Read => read(take_read_object, take_read_key);
    String => String { value: take_string_value };
    Primitive => Primitive { value: take_primitive_value;
        hint: crate::engine::vm::ToPrimitiveHint::Number };
    Exec => RegExpExec { regexp: take_exec_regexp, input: take_exec_input };
    Set => set_keyed(take_set_object, take_set_key, take_set_value) @ RegExpIteratorSet;
    raw {
        T::Complete(result) => Self::NativeRawComplete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::StringFactoryStep => StringFactory;
    Read { object, key } => read;
    Number { value } => Number;
    String { value } => String;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}
