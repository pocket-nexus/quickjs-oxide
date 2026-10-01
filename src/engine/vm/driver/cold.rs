//! One exhaustive cold-boundary dispatch; no exit is scanned by a second dispatcher.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Disposition {
    Entered,
    Complete,
    Rethrow,
    Bridge,
    Suspend(super::super::VmSuspendKind),
}

pub(super) struct Context<'a> {
    runtime: &'a Runtime,
    execution: &'a mut RunningExecution,
    id: FrameId,
    forwarded: &'a mut Option<Completion>,
    conversion: &'a mut Option<super::super::conversion_driver::ConversionTask>,
    next_operation: &'a mut u64,
    conversion_prepared: bool,
}
impl Context<'_> {
    fn complete(&mut self, completion: Completion) -> Disposition {
        let disposition = if matches!(completion, Completion::Throw(_)) {
            Disposition::Rethrow
        } else {
            Disposition::Complete
        };
        *self.forwarded = Some(completion);
        disposition
    }
    fn step(&mut self, step: CallStep) -> Disposition {
        match step {
            CallStep::Entered => Disposition::Entered,
            CallStep::Complete(completion) => self.complete(completion),
            CallStep::Bridge => Disposition::Bridge,
        }
    }
}

#[inline(never)]
// Pass the exit and its publication/conversion facts separately without constructing another dispatch payload.
#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    exit: VmAction,
    forwarded: &mut Option<Completion>,
    conversion: &mut Option<super::super::conversion_driver::ConversionTask>,
    next_operation: &mut u64,
    conversion_prepared: bool,
) -> Result<Disposition, Error> {
    let mut context = Context {
        runtime,
        execution,
        id,
        forwarded,
        conversion,
        next_operation,
        conversion_prepared,
    };
    Ok(match exit {
        VmAction::Pure(operation) => {
            let step = super::super::frame_operations::pure(
                context.runtime,
                context.execution,
                context.id,
                operation,
            )?;
            context.step(step)
        }
        VmAction::CopyData {
            target,
            source,
            excluded,
        } => {
            let step = super::super::frame_operations::copy_data(
                context.runtime,
                context.execution,
                context.id,
                usize::from(target),
                usize::from(source),
                excluded.map(usize::from),
            )?;
            context.step(step)
        }
        VmAction::HomeObject => {
            let step = super::super::frame_operations::home_object(
                context.runtime,
                context.execution,
                context.id,
            )?;
            context.step(step)
        }
        VmAction::GetSuper => {
            let step = super::super::frame_operations::get_super(
                context.runtime,
                context.execution,
                context.id,
            )?;
            context.step(step)
        }
        VmAction::BindingError {
            index,
            redeclaration,
        } => {
            let step = super::super::frame_operations::binding_error(
                context.runtime,
                context.execution,
                context.id,
                index,
                redeclaration,
            )?;
            context.step(step)
        }
        VmAction::PrivateInitialize { index, kind } => {
            let step = super::super::frame_operations::private_initialize(
                context.runtime,
                context.execution,
                context.id,
                index,
                kind,
            )?;
            context.step(step)
        }
        VmAction::PrivateAccess { source, access } => {
            let step = super::super::frame_operations::private_access(
                context.runtime,
                context.execution,
                context.id,
                source,
                access,
            )?;
            context.step(step)
        }
        VmAction::ForIn(next) => {
            let step = super::super::frame_operations::for_in(
                context.runtime,
                context.execution,
                context.id,
                next,
            )?;
            context.step(step)
        }
        VmAction::Numeric { kind, .. } => {
            let step = super::super::frame_operations::numeric(
                context.runtime,
                context.execution,
                context.id,
                kind,
            )?;
            context.step(step)
        }
        VmAction::StrictEquality(negate) => {
            let step = super::super::frame_operations::strict_equality(
                context.runtime,
                context.execution,
                context.id,
                negate,
            )?;
            context.step(step)
        }
        exit @ (VmAction::Arguments(_) | VmAction::Rest(_)) => {
            let step = super::super::frame_operations::arguments(
                context.runtime,
                context.execution,
                context.id,
                exit,
            )?;
            context.step(step)
        }
        VmAction::SetName(index) => {
            let step = super::super::frame_operations::set_name(
                context.runtime,
                context.execution,
                context.id,
                index,
            )?;
            context.step(step)
        }
        VmAction::InstantiateClosure(index) => {
            let step = super::super::frame_operations::instantiate_closure(
                context.runtime,
                context.execution,
                context.id,
                index,
            )?;
            context.step(step)
        }
        VmAction::ResetCaptured(index) => {
            let step = super::super::frame_operations::reset_captured(
                context.runtime,
                context.execution,
                context.id,
                index,
            )?;
            context.step(step)
        }
        VmAction::CloseCaptured(index) => {
            let step = super::super::frame_operations::close_captured(
                context.runtime,
                context.execution,
                context.id,
                index,
            )?;
            context.step(step)
        }
        exit @ (VmAction::Catch(_) | VmAction::DropCatch | VmAction::NipCatch) => {
            let step = super::super::frame_operations::catch(
                context.runtime,
                context.execution,
                context.id,
                exit,
            )?;
            context.step(step)
        }
        VmAction::Throw => {
            let step = super::super::frame_operations::throw(
                context.runtime,
                context.execution,
                context.id,
            )?;
            context.step(step)
        }
        VmAction::Binding {
            source,
            index,
            write,
            checked,
            keep,
        } => {
            let step = super::super::frame_operations::binding(
                context.runtime,
                context.execution,
                context.id,
                source,
                index,
                write,
                checked,
                keep,
            )?;
            context.step(step)
        }
        VmAction::LexicalUninitialized(index) => {
            let step = super::super::frame_operations::lexical_uninitialized(
                context.runtime,
                context.execution,
                context.id,
                index,
            )?;
            context.step(step)
        }
        VmAction::InitializeDerived(index) => {
            let step = super::super::frame_operations::initialize_derived(
                context.runtime,
                context.execution,
                context.id,
                index,
            )?;
            context.step(step)
        }
        VmAction::ReturnDerived(index) => {
            let step = super::super::frame_operations::return_derived(
                context.runtime,
                context.execution,
                context.id,
                index,
            )?;
            context.step(step)
        }
        VmAction::NormalizeThis => {
            let step = super::super::frame_operations::normalize_this(
                context.runtime,
                context.execution,
                context.id,
            )?;
            context.step(step)
        }
        VmAction::Call {
            arguments,
            method,
            tail,
        } => call(&mut context, arguments, method, tail)?,
        VmAction::Environment(super::super::environment_driver::Operation::Has {
            source,
            name,
        }) => with_has(&mut context, source, name)?,
        VmAction::Environment(op) => environment(&mut context, op)?,
        VmAction::DefineProperty { key, method } => define_property(&mut context, key, method)?,
        VmAction::DefineClass { name, has_heritage } => {
            define_class(&mut context, name, has_heritage)?
        }
        VmAction::ClassInitializer(mode) => class_initializer(&mut context, mode)?,
        VmAction::Construct(count) => construct(&mut context, count)?,
        VmAction::Apply(kind) => apply(&mut context, kind)?,
        VmAction::InitDerivedConstructor => init_derived_constructor(&mut context)?,
        VmAction::ConvertAdd => convert(&mut context, true, false)?,
        VmAction::ApplyEval(environment) => apply_eval(&mut context, environment)?,
        VmAction::Eval {
            arguments,
            environment,
        } => eval(&mut context, arguments, environment)?,
        VmAction::Import => import(&mut context)?,
        VmAction::Predicate(kind) => predicate(&mut context, kind)?,
        VmAction::SuperProperty(kind) => super_property(&mut context, kind)?,
        VmAction::SetProperty(key) => set_property(&mut context, key)?,
        VmAction::GetField {
            index,
            keep_receiver,
            fallthrough,
        } => get_field(&mut context, index, keep_receiver, fallthrough)?,
        VmAction::GetElement {
            keep_receiver,
            keep_key,
            fallthrough,
        } => get_element(&mut context, keep_receiver, keep_key, fallthrough)?,
        VmAction::ConvertPlus => convert(&mut context, false, false)?,
        VmAction::ConvertPropertyKey => convert(&mut context, false, true)?,
        #[cfg(all(test, feature = "profiling"))]
        exit @ VmAction::ReleaseOperand { .. } => direct(&mut context, exit)?,
        VmAction::Complete => {
            if matches!(context.forwarded, Some(Completion::Throw(_))) {
                Disposition::Rethrow
            } else {
                Disposition::Complete
            }
        }
        VmAction::Bridge => Disposition::Bridge,
        VmAction::Suspend(kind) => Disposition::Suspend(kind),
        VmAction::Materialize => {
            return Err(Error::internal("resident-only exit reached cold dispatch"));
        }
    })
}

#[inline(never)]
#[cfg(all(test, feature = "profiling"))]
fn direct(context: &mut Context<'_>, exit: VmAction) -> Result<Disposition, Error> {
    if super::super::frame_operations::complete_owned_slot(
        context.runtime,
        context.execution,
        context.id,
        exit,
    )? {
        Ok(Disposition::Entered)
    } else {
        Err(Error::internal("direct cold operation was not completed"))
    }
}

#[inline(never)]
fn call(
    context: &mut Context<'_>,
    arguments: u16,
    method: bool,
    tail: bool,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::enter_call(runtime, execution, id, arguments, method, tail)? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Ok(Disposition::Bridge),
    }
}

#[inline(never)]
fn with_has(
    context: &mut Context<'_>,
    source: crate::engine::code::bytecode::DynamicEnvironmentSource,
    name: u32,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::super::with_driver::start(runtime, execution, id, source, name)? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Ok(Disposition::Bridge),
    }
}

#[inline(never)]
fn environment(
    context: &mut Context<'_>,
    op: super::super::environment_driver::Operation,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::super::environment_driver::step(runtime, execution, id, op)? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Ok(Disposition::Bridge),
    }
}

#[inline(never)]
fn define_property(
    context: &mut Context<'_>,
    key: Option<u32>,
    method: Option<(crate::engine::code::bytecode::DefineMethodKind, bool)>,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::super::construct_driver::define_property(runtime, execution, id, key, method)? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Ok(Disposition::Bridge),
    }
}

#[inline(never)]
fn define_class(
    context: &mut Context<'_>,
    name: u32,
    has_heritage: bool,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    (*context.next_operation) = (*context.next_operation)
        .checked_add(1)
        .ok_or_else(|| Error::internal("operation identity exhausted"))?;
    match super::super::construct_driver::define_class(
        runtime,
        execution,
        id,
        name,
        has_heritage,
        *context.next_operation,
    )? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Ok(Disposition::Bridge),
    }
}

#[inline(never)]
fn class_initializer(
    context: &mut Context<'_>,
    mode: super::super::construct_driver::InitializerKind,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::super::construct_driver::initializer(runtime, execution, id, mode)? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Err(Error::internal("class initialization attempted replay")),
    }
}

#[inline(never)]
fn convert(
    context: &mut Context<'_>,
    addition: bool,
    property_key: bool,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    if !context.conversion_prepared {
        let frame = execution.frames.current_mut(id)?;
        for offset in (0..=usize::from(addition)).rev() {
            execution.slots.peek(&frame.window, offset)?;
        }
        (*context.next_operation) = (*context.next_operation)
            .checked_add(1)
            .ok_or_else(|| Error::internal("conversion identity exhausted"))?;
    }
    *context.conversion = Some(crate::engine::vm::conversion_driver::ConversionTask::start(
        runtime,
        execution,
        id,
        *context.next_operation,
        addition,
        property_key,
    )?);
    Ok(Disposition::Entered)
}

#[inline(never)]
fn apply_eval(context: &mut Context<'_>, environment: u16) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::super::eval_driver::apply(runtime, execution, id, environment)? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Ok(Disposition::Bridge),
    }
}

#[inline(never)]
fn eval(context: &mut Context<'_>, arguments: u16, environment: u16) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::super::eval_driver::step(runtime, execution, id, arguments, environment)? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Ok(Disposition::Bridge),
    }
}

#[inline(never)]
fn import(context: &mut Context<'_>) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::super::proxy_get_driver::start_import(runtime, execution, id)? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Err(Error::internal("dynamic import attempted replay")),
    }
}

#[inline(never)]
fn predicate(
    context: &mut Context<'_>,
    kind: super::super::predicate_driver::Kind,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::super::predicate_driver::start(runtime, execution, id, kind)? {
        super::super::predicate_driver::Progress::Convert(input) => {
            (*context.next_operation) = (*context.next_operation)
                .checked_add(1)
                .ok_or_else(|| Error::internal("predicate conversion identity exhausted"))?;
            *context.conversion = Some(
                super::super::conversion_driver::ConversionTask::start_predicate(
                    runtime,
                    execution,
                    id,
                    *context.next_operation,
                    input,
                )?,
            );
            Ok(Disposition::Entered)
        }
        super::super::predicate_driver::Progress::Call(CallStep::Entered) => {
            Ok(Disposition::Entered)
        }
        super::super::predicate_driver::Progress::Call(CallStep::Complete(completion)) => {
            Ok(context.complete(completion))
        }
        super::super::predicate_driver::Progress::Call(CallStep::Bridge) => {
            Err(Error::internal("predicate attempted replay"))
        }
    }
}

#[inline(never)]
fn super_property(
    context: &mut Context<'_>,
    kind: super::super::super_property_driver::Kind,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::super::super_property_driver::start(runtime, execution, id, kind)? {
        super::super::super_property_driver::Progress::Convert(input) => {
            (*context.next_operation) = (*context.next_operation)
                .checked_add(1)
                .ok_or_else(|| Error::internal("super conversion identity exhausted"))?;
            *context.conversion = Some(
                super::super::conversion_driver::ConversionTask::start_super_property(
                    runtime,
                    execution,
                    id,
                    *context.next_operation,
                    input,
                )?,
            );
            Ok(Disposition::Entered)
        }
        super::super::super_property_driver::Progress::Call(CallStep::Entered) => {
            Ok(Disposition::Entered)
        }
        super::super::super_property_driver::Progress::Call(CallStep::Complete(completion)) => {
            Ok(context.complete(completion))
        }
        super::super::super_property_driver::Progress::Call(CallStep::Bridge) => {
            Err(Error::internal("super property attempted replay"))
        }
    }
}

#[inline(never)]
fn set_property(context: &mut Context<'_>, key: Option<u32>) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    let frame = execution.frames.current_mut(id)?;
    if key.is_none() && matches!(execution.slots.peek(&frame.window, 1)?, JsValue::Object(_)) {
        (*context.next_operation) = (*context.next_operation)
            .checked_add(1)
            .ok_or_else(|| Error::internal("write conversion identity exhausted"))?;
        *context.conversion = Some(
            super::super::conversion_driver::ConversionTask::start_property_write(
                runtime,
                execution,
                id,
                *context.next_operation,
            )?,
        );
        return Ok(Disposition::Entered);
    }
    match super::super::property_write_driver::write(runtime, execution, id, key)? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Ok(Disposition::Bridge),
    }
}

#[inline(never)]
fn get_field(
    context: &mut Context<'_>,
    index: u32,
    keep_receiver: bool,
    fallthrough: super::super::execute::FallthroughPc,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    match super::super::property_driver::read(
        runtime,
        execution,
        id,
        super::super::property_driver::ReadKey::Static(index),
        keep_receiver,
        fallthrough,
    )? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Ok(Disposition::Bridge),
    }
}

#[inline(never)]
fn get_element(
    context: &mut Context<'_>,
    keep_receiver: bool,
    keep_key: bool,
    fallthrough: super::super::execute::FallthroughPc,
) -> Result<Disposition, Error> {
    let runtime = context.runtime;
    let execution = &mut *context.execution;
    let id = context.id;

    let frame = execution.frames.current_mut(id)?;
    if !matches!(
        execution.slots.peek(&frame.window, 1)?,
        JsValue::Null | JsValue::Undefined
    ) && matches!(execution.slots.peek(&frame.window, 0)?, JsValue::Object(_))
    {
        (*context.next_operation) = (*context.next_operation)
            .checked_add(1)
            .ok_or_else(|| Error::internal("property conversion identity exhausted"))?;
        *context.conversion = Some(
            super::super::conversion_driver::ConversionTask::start_property_read(
                runtime,
                execution,
                id,
                *context.next_operation,
                keep_receiver,
                keep_key,
            )?,
        );
        return Ok(Disposition::Entered);
    }
    match super::super::property_driver::read(
        runtime,
        execution,
        id,
        super::super::property_driver::ReadKey::Computed { keep_key },
        keep_receiver,
        fallthrough,
    )? {
        CallStep::Entered => Ok(Disposition::Entered),
        CallStep::Complete(completion) => Ok(context.complete(completion)),
        CallStep::Bridge => Ok(Disposition::Bridge),
    }
}

#[inline(never)]
fn construct(context: &mut Context<'_>, count: u16) -> Result<Disposition, Error> {
    *context.next_operation = context
        .next_operation
        .checked_add(1)
        .ok_or_else(|| Error::internal("operation identity exhausted"))?;
    let step = super::super::construct_driver::enter(
        context.runtime,
        context.execution,
        context.id,
        count,
        *context.next_operation,
    )?;
    Ok(context.step(step))
}

#[inline(never)]
fn apply(
    context: &mut Context<'_>,
    kind: crate::engine::code::bytecode::ApplyKind,
) -> Result<Disposition, Error> {
    *context.next_operation = context
        .next_operation
        .checked_add(1)
        .ok_or_else(|| Error::internal("operation identity exhausted"))?;
    let step = super::super::apply_driver::step(
        context.runtime,
        context.execution,
        context.id,
        kind,
        *context.next_operation,
    )?;
    Ok(context.step(step))
}

#[inline(never)]
fn init_derived_constructor(context: &mut Context<'_>) -> Result<Disposition, Error> {
    *context.next_operation = context
        .next_operation
        .checked_add(1)
        .ok_or_else(|| Error::internal("operation identity exhausted"))?;
    let step = super::super::construct_driver::enter_default_derived(
        context.runtime,
        context.execution,
        context.id,
        *context.next_operation,
    )?;
    Ok(context.step(step))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cold_dispatch_carries_only_disposition_not_completion_owners() {
        assert!(size_of::<Disposition>() <= 8);
        assert!(size_of::<VmAction>() <= 32);
    }
}
