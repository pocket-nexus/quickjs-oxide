//! Flatten owns nested source frames and commits each output before visiting the next element.
use super::{ARRAY_FLATTEN_FRAME_LIMIT, ArrayFlattenFrame};
#[cfg(test)]
use crate::engine::value::Value;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::ArrayFlattenKind,
    heap::ContextId,
    object::{
        CallableRef, DescriptorField, ObjectRef, OwnedPropertyDescriptor, PropertyKey,
        operations::InternalDefineResult,
    },
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum FlattenStep {
    Complete(Completion),
    Read { resume: FlattenResume },
    Number { resume: FlattenResume },
    Has { resume: FlattenResume },
    Species { resume: FlattenResume },
    Call { resume: FlattenResume },
    Define { resume: FlattenResume },
}
enum Phase {
    Length,
    LengthNumber,
    Depth,
    Species,
    Has,
    Read,
    Mapper,
    NestedLength,
    NestedNumber,
    Define,
}
pub(crate) struct FlattenResume(Box<FlattenResumeState>);
impl std::ops::Deref for FlattenResume {
    type Target = FlattenResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for FlattenResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<FlattenResume>() <= 8);
pub(crate) struct FlattenResumeState {
    runtime: Runtime,
    pending_effect: FlattenStepPending,
    realm: ContextId,
    kind: ArrayFlattenKind,
    phase: Phase,
    source: ObjectRef,
    source_index: u64,
    source_length: u64,
    depth: i32,
    argument: JsValue,
    mapper: Option<CallableRef>,
    mapper_this: JsValue,
    target: Option<ObjectRef>,
    frames: Vec<ArrayFlattenFrame>,
    element: JsValue,
    target_index: u64,
    target_limit: u64,
    frame_limit: usize,
    return_count: bool,
}
impl Drop for FlattenResumeState {
    fn drop(&mut self) {
        for value in [
            std::mem::replace(&mut self.argument, JsValue::Undefined),
            std::mem::replace(&mut self.mapper_this, JsValue::Undefined),
            std::mem::replace(&mut self.element, JsValue::Undefined),
        ] {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.pending_effect.number_value.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.pending_effect.call_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(values) = self.pending_effect.call_arguments.take() {
            for value in values {
                let _ = self.runtime.release_jsvalue(value);
            }
        }
    }
}
impl FlattenStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ArrayFlattenKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array flatten requires generic invocation",
            ));
        };
        let source =
            match runtime.native_to_object_jsvalue(realm, runtime.dup_jsvalue(this_value)?)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(Self::Complete(Completion::Throw(
                        runtime.into_jsvalue(value)?,
                    )));
                }
            };
        let mut resume = FlattenResume(Box::new(FlattenResumeState {
            runtime: runtime.clone(),
            pending_effect: FlattenStepPending::default(),
            realm,
            kind,
            phase: Phase::Length,
            source,
            source_index: 0,
            source_length: 0,
            depth: 1,
            argument: JsValue::Undefined,
            mapper: None,
            mapper_this: JsValue::Undefined,
            target: None,
            frames: Vec::new(),
            element: JsValue::Undefined,
            target_index: 0,
            target_limit: (1_u64 << 53) - 1,
            frame_limit: ARRAY_FLATTEN_FRAME_LIMIT,
            return_count: false,
        }));
        if let Some(value) = arguments.readable.first() {
            resume.0.argument = runtime.dup_jsvalue(value)?;
        }
        if arguments.actual_arg_count > 1
            && let Some(value) = arguments.readable.get(1)
        {
            resume.0.mapper_this = runtime.dup_jsvalue(value)?;
        }
        let key = runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
        Ok(Self::request_read(resume.0.source.clone(), key, resume))
    }
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(super) fn start_into(
        runtime: &Runtime,
        realm: ContextId,
        target: ObjectRef,
        source: ObjectRef,
        length: u64,
        depth: i32,
        mapper: Option<CallableRef>,
        mapper_this: Value,
        target_limit: u64,
        frame_limit: usize,
    ) -> Result<Self, RuntimeError> {
        FlattenResume(Box::new(FlattenResumeState {
            runtime: runtime.clone(),
            pending_effect: FlattenStepPending::default(),
            realm,
            kind: ArrayFlattenKind::Flat,
            phase: Phase::Species,
            source,
            source_index: 0,
            source_length: length,
            depth,
            argument: JsValue::Undefined,
            mapper,
            mapper_this: runtime.into_jsvalue(mapper_this)?,
            target: Some(target),
            frames: Vec::new(),
            element: JsValue::Undefined,
            target_index: 0,
            target_limit,
            frame_limit,
            return_count: true,
        }))
        .begin(runtime)
    }
}
impl FlattenResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<FlattenStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(FlattenStep::Complete(Completion::Throw(value))),
        };
        match self.0.phase {
            Phase::Length => {
                self.0.phase = Phase::LengthNumber;
                Ok(FlattenStep::request_number(value, self))
            }
            Phase::Species => {
                let JsValue::Object(id) = value else {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant(
                        "ArraySpeciesCreate returned a primitive for flatten",
                    ));
                };
                self.0.target = Some(ObjectRef::from_owned_handle(runtime.clone(), id));
                self.begin(runtime)
            }
            Phase::Read => {
                runtime.release_jsvalue(std::mem::replace(&mut self.0.element, value))?;
                let apply = self
                    .0
                    .frames
                    .last()
                    .ok_or(RuntimeError::Invariant("flatten current frame missing"))?
                    .apply_mapper;
                if apply {
                    self.0.phase = Phase::Mapper;
                    self.0.pending_effect.call_callable = Some(
                        self.0
                            .mapper
                            .as_ref()
                            .ok_or(RuntimeError::Invariant("flatten mapper missing"))?
                            .clone(),
                    );
                    self.0.pending_effect.call_receiver =
                        Some(runtime.dup_jsvalue(&self.0.mapper_this)?);
                    let value = std::mem::replace(&mut self.0.element, JsValue::Undefined);
                    self.0.pending_effect.call_arguments = Some(vec![
                        value,
                        crate::engine::value::number::operations::Number::compact(
                            self.0.source_index as f64,
                        )
                        .into(),
                        JsValue::Object(self.0.source.clone().into_handle()),
                    ]);
                    return Ok(FlattenStep::Call { resume: self });
                }
                let value = std::mem::replace(&mut self.0.element, JsValue::Undefined);
                self.visit(runtime, value)
            }
            Phase::Mapper => self.visit(runtime, value),
            Phase::NestedLength => {
                self.0.phase = Phase::NestedNumber;
                Ok(FlattenStep::request_number(value, self))
            }
            _ => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "Array flatten value phase mismatch",
                ))
            }
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<FlattenStep, RuntimeError> {
        let number = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(FlattenStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        match self.0.phase {
            Phase::LengthNumber => {
                self.0.source_length = Runtime::length_from_number(number);
                if self.0.kind == ArrayFlattenKind::FlatMap {
                    let mapper = match &self.0.argument {
                        JsValue::Object(id) => runtime.as_callable_object(*id)?,
                        _ => None,
                    };
                    self.0.mapper = Some(mapper.ok_or_else(|| {
                        RuntimeError::Engine(crate::engine::api::Error::new(
                            crate::engine::api::ErrorKind::Type,
                            "not a function",
                        ))
                    })?);
                } else if !matches!(self.0.argument, JsValue::Undefined) {
                    self.0.phase = Phase::Depth;
                    return Ok(FlattenStep::request_number(
                        runtime.dup_jsvalue(&self.0.argument)?,
                        self,
                    ));
                }
                self.species()
            }
            Phase::Depth => {
                self.0.depth = crate::engine::value::number::to_int32_sat(number);
                self.species()
            }
            Phase::NestedNumber => {
                if self.0.frames.len() >= self.0.frame_limit {
                    return self.overflow(runtime);
                }
                let element = std::mem::replace(&mut self.0.element, JsValue::Undefined);
                let JsValue::Object(id) = element else {
                    runtime.release_jsvalue(element)?;
                    return Err(RuntimeError::Invariant("flatten nested object missing"));
                };
                let source = ObjectRef::from_owned_handle(runtime.clone(), id);
                self.0.frames.push(ArrayFlattenFrame {
                    source,
                    length: Runtime::length_from_number(number),
                    next_index: 0,
                    depth: self.0.depth - 1,
                    apply_mapper: false,
                });
                self.next(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "Array flatten number phase mismatch",
            )),
        }
    }
    fn species(mut self) -> Result<FlattenStep, RuntimeError> {
        self.0.phase = Phase::Species;
        Ok(FlattenStep::request_species(self.0.source.clone(), self))
    }
    fn overflow(&self, runtime: &Runtime) -> Result<FlattenStep, RuntimeError> {
        Ok(FlattenStep::Complete(Completion::Throw(
            runtime.new_native_error_jsvalue(
                self.0.realm,
                NativeErrorKind::Internal,
                "stack overflow",
            )?,
        )))
    }
    fn begin(mut self, runtime: &Runtime) -> Result<FlattenStep, RuntimeError> {
        if self.0.frame_limit == 0 {
            return self.overflow(runtime);
        }
        self.0.frames.push(ArrayFlattenFrame {
            source: self.0.source.clone(),
            length: self.0.source_length,
            next_index: 0,
            depth: self.0.depth,
            apply_mapper: self.0.mapper.is_some(),
        });
        self.next(runtime)
    }
    fn next(mut self, runtime: &Runtime) -> Result<FlattenStep, RuntimeError> {
        loop {
            let Some(frame) = self.0.frames.last_mut() else {
                let value = if self.0.return_count {
                    crate::engine::value::number::operations::Number::compact(
                        self.0.target_index as f64,
                    )
                    .into()
                } else {
                    JsValue::Object(
                        self.0
                            .target
                            .take()
                            .ok_or(RuntimeError::Invariant("flatten target missing"))?
                            .into_handle(),
                    )
                };
                return Ok(FlattenStep::Complete(Completion::Return(value)));
            };
            if frame.next_index == frame.length {
                self.0.frames.pop();
                continue;
            }
            self.0.source = frame.source.clone();
            self.0.source_index = frame.next_index;
            frame.next_index += 1;
            self.0.depth = frame.depth;
            self.0.phase = Phase::Has;
            return Ok(FlattenStep::request_has(
                self.0.source.clone(),
                runtime.property_key_for_index(self.0.source_index)?,
                self,
            ));
        }
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<FlattenStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Has) {
            return Err(RuntimeError::Invariant(
                "Array flatten boolean phase mismatch",
            ));
        }
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(FlattenStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        if !value {
            return self.next(runtime);
        }
        self.0.phase = Phase::Read;
        Ok(FlattenStep::request_read(
            self.0.source.clone(),
            runtime.property_key_for_index(self.0.source_index)?,
            self,
        ))
    }
    fn visit(mut self, runtime: &Runtime, element: JsValue) -> Result<FlattenStep, RuntimeError> {
        runtime.release_jsvalue(std::mem::replace(&mut self.0.element, element))?;
        let flatten = if self.0.depth > 0 {
            match runtime.internal_is_array_jsvalue(self.0.realm, &self.0.element)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(FlattenStep::Complete(Completion::Throw(
                        runtime.into_jsvalue(value)?,
                    )));
                }
            }
        } else {
            false
        };
        if flatten {
            let JsValue::Object(id) = self.0.element else {
                return Err(RuntimeError::Invariant(
                    "IsArray accepted primitive flatten element",
                ));
            };
            let object = ObjectRef::from_borrowed_handle(runtime.clone(), id)?;
            self.0.phase = Phase::NestedLength;
            return Ok(FlattenStep::request_read(
                object,
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?,
                self,
            ));
        }
        if self.0.target_index >= self.0.target_limit {
            return Ok(FlattenStep::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    self.0.realm,
                    NativeErrorKind::Type,
                    "Array too long",
                )?,
            )));
        }
        let mut descriptor = OwnedPropertyDescriptor::new(runtime);
        descriptor.value =
            DescriptorField::Present(std::mem::replace(&mut self.0.element, JsValue::Undefined));
        descriptor.writable = DescriptorField::Present(true);
        descriptor.enumerable = DescriptorField::Present(true);
        descriptor.configurable = DescriptorField::Present(true);
        self.0.phase = Phase::Define;
        Ok(FlattenStep::request_define(
            self.0
                .target
                .clone()
                .ok_or(RuntimeError::Invariant("flatten target missing"))?,
            runtime.property_key_for_index(self.0.target_index)?,
            descriptor,
            self,
        ))
    }
    pub(crate) fn defined(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<FlattenStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Define) {
            return Err(RuntimeError::Invariant(
                "Array flatten define phase mismatch",
            ));
        }
        if let Some(value) = runtime.finish_create_indexed_data_property(
            self.0.realm,
            self.0.target_index,
            result,
        )? {
            return Ok(FlattenStep::Complete(Completion::Throw(
                runtime.into_jsvalue(value)?,
            )));
        }
        self.0.target_index += 1;
        self.next(runtime)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: FlattenStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            FlattenStep::Complete(result) => return Ok(result),
            FlattenStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_property_in_realm(realm, &object, &key)?,
                )?
            }
            FlattenStep::Number { mut resume } => {
                let value = resume.take_number_value();
                resume.number(runtime, runtime.native_to_number_jsvalue(realm, value)?)?
            }
            FlattenStep::Has { mut resume } => {
                let object = resume.take_has_object();
                let key = resume.take_has_key();
                resume.boolean(
                    runtime,
                    runtime.internal_has_property(realm, &object, &key)?,
                )?
            }
            FlattenStep::Species { mut resume } => {
                let source = resume.take_species_source();
                resume.resume(
                    runtime,
                    super::species::finish(
                        runtime,
                        realm,
                        super::species::SpeciesStep::start(runtime, realm, &source, 0)?,
                    )?,
                )?
            }
            FlattenStep::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let receiver = resume.take_call_receiver();
                let arguments = resume.take_call_arguments();
                resume.resume(
                    runtime,
                    runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                )?
            }
            FlattenStep::Define { mut resume } => {
                let object = resume.take_define_object();
                let key = resume.take_define_key();
                let descriptor = resume.take_define_descriptor();
                resume.defined(
                    runtime,
                    runtime.internal_define_owned_property(realm, &object, &key, descriptor)?,
                )?
            }
        };
    }
}

#[derive(Default)]
struct FlattenStepPending {
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
    number_value: Option<JsValue>,
    has_object: Option<ObjectRef>,
    has_key: Option<PropertyKey>,
    species_source: Option<ObjectRef>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    define_object: Option<ObjectRef>,
    define_key: Option<PropertyKey>,
    define_descriptor: Option<OwnedPropertyDescriptor>,
}
impl FlattenStep {
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: FlattenResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_number(value: JsValue, mut resume: FlattenResume) -> Self {
        resume.0.pending_effect.number_value = Some(value);
        Self::Number { resume }
    }
    pub(crate) fn request_has(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: FlattenResume,
    ) -> Self {
        resume.0.pending_effect.has_object = Some(object);
        resume.0.pending_effect.has_key = Some(key);
        Self::Has { resume }
    }
    pub(crate) fn request_species(source: ObjectRef, mut resume: FlattenResume) -> Self {
        resume.0.pending_effect.species_source = Some(source);
        Self::Species { resume }
    }
    pub(crate) fn request_define(
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OwnedPropertyDescriptor,
        mut resume: FlattenResume,
    ) -> Self {
        resume.0.pending_effect.define_object = Some(object);
        resume.0.pending_effect.define_key = Some(key);
        resume.0.pending_effect.define_descriptor = Some(descriptor);
        Self::Define { resume }
    }
}
impl FlattenResume {
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("FlattenStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("FlattenStep Read key")
    }
    pub(crate) fn take_number_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .number_value
            .take()
            .expect("FlattenStep Number value")
    }
    pub(crate) fn take_has_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .has_object
            .take()
            .expect("FlattenStep Has object")
    }
    pub(crate) fn take_has_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .has_key
            .take()
            .expect("FlattenStep Has key")
    }
    pub(crate) fn take_species_source(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .species_source
            .take()
            .expect("FlattenStep Species source")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("FlattenStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("FlattenStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("FlattenStep Call arguments")
    }
    pub(crate) fn take_define_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .define_object
            .take()
            .expect("FlattenStep Define object")
    }
    pub(crate) fn take_define_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .define_key
            .take()
            .expect("FlattenStep Define key")
    }
    pub(crate) fn take_define_descriptor(&mut self) -> OwnedPropertyDescriptor {
        self.0
            .pending_effect
            .define_descriptor
            .take()
            .expect("FlattenStep Define descriptor")
    }
}
const _: () = assert!(std::mem::size_of::<FlattenStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<FlattenStep>() <= 64);
