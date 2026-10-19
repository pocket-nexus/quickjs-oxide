//! The generator and async-function intrinsics: `%GeneratorFunction%` and its
//! prototypes with `%GeneratorPrototype%`'s `next`, `return` and `throw`;
//! `%AsyncFunction%`; the async generator counterparts, whose natives are in
//! `vm::async_generator`.

use super::{Native, methods, plain_object, to_string_tag};
use crate::heap::HeapError;
use crate::heap::object::InternalTag;
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::suspend::{self, Resumption};
use crate::vm::{Cx, Throw, error, iter};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let function_prototype = cx.realm.intrinsic(Intrinsic::FunctionPrototype);
    let iterator_prototype = cx.realm.intrinsic(Intrinsic::IteratorPrototype);
    let names = cx.realm.names;
    let constructor_link = |cx: &mut Cx<'_>, constructor: ObjectId, prototype: ObjectId| {
        cx.heap.add_property(
            constructor,
            Names::key(names.prototype),
            PropertyFlags::data(false, false, false),
            Value::object(prototype),
        )?;
        cx.heap.add_property(
            prototype,
            Names::key(names.constructor),
            PropertyFlags::data(false, false, true),
            Value::object(constructor),
        )
    };

    // %GeneratorFunction%, %GeneratorFunction.prototype% and
    // %GeneratorPrototype%.
    let generator_function_prototype = plain_object(cx, Some(function_prototype), 4)?;
    cx.set_intrinsic(
        Intrinsic::GeneratorFunctionPrototype,
        generator_function_prototype,
    );
    let generator_prototype = plain_object(cx, Some(iterator_prototype), 4)?;
    cx.set_intrinsic(Intrinsic::GeneratorPrototype, generator_prototype);
    methods(
        cx,
        generator_prototype,
        &[
            Native::GeneratorNext,
            Native::GeneratorReturn,
            Native::GeneratorThrow,
        ],
    )?;
    to_string_tag(cx, generator_prototype, "Generator")?;
    to_string_tag(cx, generator_function_prototype, "GeneratorFunction")?;
    let generator_function = super::native_function(cx, Native::GeneratorFunctionConstructor)?;
    let function = cx.realm.intrinsic(Intrinsic::Function);
    cx.heap.set_prototype(generator_function, Some(function))?;
    constructor_link(cx, generator_function, generator_function_prototype)?;
    // QuickJS links `%GeneratorPrototype%` after the constructor.
    cx.heap.add_property(
        generator_function_prototype,
        Names::key(names.prototype),
        PropertyFlags::data(false, false, true),
        Value::object(generator_prototype),
    )?;
    cx.heap.add_property(
        generator_prototype,
        Names::key(names.constructor),
        PropertyFlags::data(false, false, true),
        Value::object(generator_function_prototype),
    )?;
    cx.set_intrinsic(Intrinsic::GeneratorFunction, generator_function);

    // %AsyncFunction% and %AsyncFunction.prototype%.
    let async_function_prototype = plain_object(cx, Some(function_prototype), 4)?;
    cx.set_intrinsic(Intrinsic::AsyncFunctionPrototype, async_function_prototype);
    to_string_tag(cx, async_function_prototype, "AsyncFunction")?;
    let async_function = super::native_function(cx, Native::AsyncFunctionConstructor)?;
    cx.heap.set_prototype(async_function, Some(function))?;
    constructor_link(cx, async_function, async_function_prototype)?;
    cx.set_intrinsic(Intrinsic::AsyncFunction, async_function);

    // %AsyncIteratorPrototype%, %AsyncGeneratorFunction%,
    // %AsyncGeneratorFunction.prototype% and %AsyncGeneratorPrototype%.
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let async_iterator_prototype = plain_object(cx, Some(object_prototype), 4)?;
    cx.set_intrinsic(Intrinsic::AsyncIteratorPrototype, async_iterator_prototype);
    let async_iterator = cx.realm.symbols.async_iterator;
    super::symbol_method(
        cx,
        async_iterator_prototype,
        async_iterator,
        Native::AsyncIteratorPrototypeAsyncIterator,
        PropertyFlags::data(true, false, true),
    )?;
    let async_generator_function_prototype = plain_object(cx, Some(function_prototype), 4)?;
    cx.set_intrinsic(
        Intrinsic::AsyncGeneratorFunctionPrototype,
        async_generator_function_prototype,
    );
    let async_generator_prototype = plain_object(cx, Some(async_iterator_prototype), 4)?;
    cx.set_intrinsic(
        Intrinsic::AsyncGeneratorPrototype,
        async_generator_prototype,
    );
    methods(
        cx,
        async_generator_prototype,
        &[
            Native::AsyncGeneratorNext,
            Native::AsyncGeneratorReturn,
            Native::AsyncGeneratorThrow,
        ],
    )?;
    to_string_tag(cx, async_generator_prototype, "AsyncGenerator")?;
    to_string_tag(
        cx,
        async_generator_function_prototype,
        "AsyncGeneratorFunction",
    )?;
    let async_generator_function =
        super::native_function(cx, Native::AsyncGeneratorFunctionConstructor)?;
    cx.heap
        .set_prototype(async_generator_function, Some(function))?;
    constructor_link(
        cx,
        async_generator_function,
        async_generator_function_prototype,
    )?;
    cx.heap.add_property(
        async_generator_function_prototype,
        Names::key(names.prototype),
        PropertyFlags::data(false, false, true),
        Value::object(async_generator_prototype),
    )?;
    cx.heap.add_property(
        async_generator_prototype,
        Names::key(names.constructor),
        PropertyFlags::data(false, false, true),
        Value::object(async_generator_function_prototype),
    )?;
    cx.set_intrinsic(Intrinsic::AsyncGeneratorFunction, async_generator_function);
    Ok(())
}

pub(super) fn generator_function_constructor(
    cx: &mut Cx<'_>,
    site: CallSite,
) -> Result<Value, Throw> {
    crate::vm::compile::function_constructor(cx, site, crate::vm::compile::FunctionKind::Generator)
}

pub(super) fn async_function_constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    crate::vm::compile::function_constructor(cx, site, crate::vm::compile::FunctionKind::Async)
}

pub(super) fn async_generator_function_constructor(
    cx: &mut Cx<'_>,
    site: CallSite,
) -> Result<Value, Throw> {
    crate::vm::compile::function_constructor(
        cx,
        site,
        crate::vm::compile::FunctionKind::AsyncGenerator,
    )
}

/// `%AsyncIteratorPrototype%[Symbol.asyncIterator]`: `this`.
pub(super) fn async_iterator_prototype_async_iterator(
    cx: &mut Cx<'_>,
    site: CallSite,
) -> Result<Value, Throw> {
    Ok(site.this(cx))
}

/// `GeneratorValidate`: the receiver as a generator that is not running.
fn this_generator(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object) if cx.heap.internal_tag(object) == InternalTag::Generator => {
            if suspend::state(cx, object) == suspend::EXECUTING {
                return Err(error::type_error(cx, "cannot invoke a running generator"));
            }
            Ok(object)
        }
        _ => Err(error::type_error(cx, "not a generator")),
    }
}

/// Resumes generator `generator` and turns how it stopped into the result:
/// `{ value, done: false }` at a `yield`, the delegate's result object at a
/// `yield*`, `{ value, done: true }` on completion.
fn drive(cx: &mut Cx<'_>, generator: ObjectId, resumption: Resumption) -> Result<Value, Throw> {
    let result = suspend::resume(cx, generator, resumption);
    if suspend::state(cx, generator) == suspend::YIELD {
        let value = result?;
        if yielded_star(cx, generator) {
            return Ok(value);
        }
        return iter::result_object(cx, value, false);
    }
    suspend::complete(cx, generator);
    let value = result?;
    iter::result_object(cx, value, true)
}

/// Whether the generator is suspended at a `yield*`.
fn yielded_star(cx: &Cx<'_>, generator: ObjectId) -> bool {
    let crate::heap::object::InternalData::Frame(saved) = cx.heap.internal(generator).data else {
        return false;
    };
    let unit = cx.unit(crate::bytecode::CodeUnitId(saved.unit));
    unit.code[saved.pc as usize - 1].op == crate::bytecode::Opcode::YieldStar
}

/// `%GeneratorPrototype%.next(value)`.
pub(super) fn next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let generator = this_generator(cx, &site)?;
    let value = site.arg(cx, 0);
    match suspend::state(cx, generator) {
        suspend::COMPLETED => iter::result_object(cx, Value::UNDEFINED, true),
        suspend::START => drive(cx, generator, Resumption::Start),
        _ => drive(cx, generator, Resumption::Next(value)),
    }
}

/// `%GeneratorPrototype%.return(value)`.
pub(super) fn return_(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let generator = this_generator(cx, &site)?;
    let value = site.arg(cx, 0);
    match suspend::state(cx, generator) {
        suspend::COMPLETED | suspend::START => {
            suspend::complete(cx, generator);
            iter::result_object(cx, value, true)
        }
        _ => drive(cx, generator, Resumption::Return(value)),
    }
}

/// `%GeneratorPrototype%.throw(exception)`.
pub(super) fn throw(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let generator = this_generator(cx, &site)?;
    let value = site.arg(cx, 0);
    match suspend::state(cx, generator) {
        suspend::COMPLETED | suspend::START => {
            suspend::complete(cx, generator);
            Err(Throw(value))
        }
        _ => drive(cx, generator, Resumption::Throw(value)),
    }
}

// ---- %AsyncFromSyncIteratorPrototype% ----

/// `CreateAsyncFromSyncIterator(syncIteratorRecord)`: internal slots the
/// sync iterator and its `next`.
pub(crate) fn async_from_sync_iterator(
    cx: &mut Cx<'_>,
    iterator: Value,
    next: Value,
) -> Result<Value, Throw> {
    let prototype = cx
        .realm
        .intrinsic(Intrinsic::AsyncFromSyncIteratorPrototype);
    cx.heap
        .alloc_internal(
            crate::heap::object::ObjectKind::Builtin,
            InternalTag::AsyncFromSyncIterator,
            Some(prototype),
            0,
            &[iterator, next],
            Default::default(),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

pub(super) fn install_async_from_sync(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let async_iterator_prototype = cx.realm.intrinsic(Intrinsic::AsyncIteratorPrototype);
    let prototype = plain_object(cx, Some(async_iterator_prototype), 4)?;
    cx.set_intrinsic(Intrinsic::AsyncFromSyncIteratorPrototype, prototype);
    methods(
        cx,
        prototype,
        &[
            Native::AsyncFromSyncNext,
            Native::AsyncFromSyncReturn,
            Native::AsyncFromSyncThrow,
        ],
    )
}

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

/// The receiver's sync iterator and `next`, and a fresh capability.
fn async_from_sync_parts(
    cx: &mut Cx<'_>,
    site: &CallSite,
) -> Result<(Value, Value, [Value; 3]), Throw> {
    let constructor = Value::object(cx.realm.intrinsic(Intrinsic::Promise));
    let capability = crate::vm::jobs::new_capability(cx, constructor)?;
    let this = site
        .this(cx)
        .as_object()
        .expect("an async-from-sync iterator");
    let iterator = cx.heap.internal_slot(this, 0);
    let next = cx.heap.internal_slot(this, 1);
    Ok((iterator, next, capability))
}

/// Rejects `capability` with the thrown value and returns its promise.
fn reject_with(
    cx: &mut Cx<'_>,
    capability: [Value; 3],
    Throw(reason): Throw,
) -> Result<Value, Throw> {
    root(cx, reason);
    crate::vm::call(cx, capability[2], Value::UNDEFINED, &[reason])?;
    Ok(capability[0])
}

/// `AsyncFromSyncIteratorContinuation(result, capability, syncRecord,
/// closeOnRejection)`.
fn continuation(
    cx: &mut Cx<'_>,
    result: Value,
    capability: [Value; 3],
    iterator: Value,
    close_on_rejection: bool,
) -> Result<Value, Throw> {
    let Some(result_object) = result.as_object() else {
        let error = error::type_error(cx, "iterator must return an object");
        return reject_with(cx, capability, error);
    };
    root(cx, result);
    let names = cx.realm.names;
    let outcome = (|| {
        let done = crate::vm::object::get_property(cx, result_object, Names::key(names.done))?;
        let done = crate::vm::ops::to_boolean(cx.heap, done);
        let value = crate::vm::object::get_property(cx, result_object, Names::key(names.value))?;
        Ok((done, value))
    })();
    let (done, value) = match outcome {
        Ok(parts) => parts,
        Err(throw) => return reject_with(cx, capability, throw),
    };
    root(cx, value);
    let constructor = Value::object(cx.realm.intrinsic(Intrinsic::Promise));
    let wrapper = match crate::vm::jobs::promise_resolve(cx, constructor, value) {
        Ok(wrapper) => wrapper,
        Err(throw) => {
            if !done && close_on_rejection {
                let record = iter::Iter::Protocol {
                    iterator,
                    next: Value::UNDEFINED,
                };
                let _ = iter::close(cx, &record, Err(throw));
            }
            return reject_with(cx, capability, throw);
        }
    };
    root(cx, wrapper);
    let data = super::native_data(cx, &[Value::bool(done), iterator])?;
    root(cx, Value::object(data));
    let on_fulfilled = super::native_closure(cx, Native::AsyncFromSyncUnwrap, data)?;
    root(cx, Value::object(on_fulfilled));
    let on_rejected = if done || !close_on_rejection {
        Value::UNDEFINED
    } else {
        let closer = super::native_closure(cx, Native::AsyncFromSyncCloseRejected, data)?;
        Value::object(closer)
    };
    root(cx, on_rejected);
    crate::vm::jobs::perform_then(
        cx,
        wrapper.as_object().expect("a promise"),
        Value::object(on_fulfilled),
        on_rejected,
        Some(capability),
        Value::UNDEFINED,
    )?;
    Ok(capability[0])
}

pub(super) fn async_from_sync_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (iterator, next, capability) = async_from_sync_parts(cx, &site)?;
    let args: Vec<Value> = site.arg_list(cx);
    let result = match crate::vm::call(cx, next, iterator, &args) {
        Ok(result) => result,
        Err(throw) => return reject_with(cx, capability, throw),
    };
    continuation(cx, result, capability, iterator, true)
}

pub(super) fn async_from_sync_return(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (iterator, _, capability) = async_from_sync_parts(cx, &site)?;
    let key = Names::key(cx.realm.names.return_);
    let method = match crate::vm::object::get_method(cx, iterator, key) {
        Ok(method) => method,
        Err(throw) => return reject_with(cx, capability, throw),
    };
    let Some(method) = method else {
        let value = site.arg(cx, 0);
        let result = iter::result_object(cx, value, true)?;
        root(cx, result);
        crate::vm::call(cx, capability[1], Value::UNDEFINED, &[result])?;
        return Ok(capability[0]);
    };
    let args: Vec<Value> = site.arg_list(cx);
    let result = match crate::vm::call(cx, method, iterator, &args) {
        Ok(result) => result,
        Err(throw) => return reject_with(cx, capability, throw),
    };
    continuation(cx, result, capability, iterator, false)
}

pub(super) fn async_from_sync_throw(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (iterator, _, capability) = async_from_sync_parts(cx, &site)?;
    let key = Names::key(cx.realm.names.throw);
    let method = match crate::vm::object::get_method(cx, iterator, key) {
        Ok(method) => method,
        Err(throw) => return reject_with(cx, capability, throw),
    };
    let Some(method) = method else {
        // The protocol was violated: close the iterator, then reject.
        let record = iter::Iter::Protocol {
            iterator,
            next: Value::UNDEFINED,
        };
        if let Err(throw) = iter::close(cx, &record, Ok(())) {
            return reject_with(cx, capability, throw);
        }
        let error = error::type_error(cx, "throw is not a method");
        return reject_with(cx, capability, error);
    };
    let args: Vec<Value> = site.arg_list(cx);
    let result = match crate::vm::call(cx, method, iterator, &args) {
        Ok(result) => result,
        Err(throw) => return reject_with(cx, capability, throw),
    };
    continuation(cx, result, capability, iterator, true)
}

/// The continuation's fulfillment handler: `{ value, done }`.
pub(super) fn async_from_sync_unwrap(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let done = site.data(cx, 0) == Value::TRUE;
    let value = site.arg(cx, 0);
    iter::result_object(cx, value, done)
}

/// The continuation's rejection handler when it closes the sync iterator.
pub(super) fn async_from_sync_close_rejected(
    cx: &mut Cx<'_>,
    site: CallSite,
) -> Result<Value, Throw> {
    let iterator = site.data(cx, 1);
    let reason = site.arg(cx, 0);
    let record = iter::Iter::Protocol {
        iterator,
        next: Value::UNDEFINED,
    };
    iter::close(cx, &record, Err(Throw(reason)))?;
    Err(Throw(reason))
}
