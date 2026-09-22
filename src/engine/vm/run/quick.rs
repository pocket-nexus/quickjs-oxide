//! Guarded dispatch over authenticated words, expanded in the running loop.
//!
//! Successful bodies enter the caller's completion continuation directly. There
//! is no intermediate Result<Outcome, Error> to construct and classify. Scalar
//! and certified-span semantics still live only in the shared hot/fusion bodies.

/// `finish!()` and `finish_span!(usize)` must publish the resume PC, record the
/// appropriate logical instructions, and continue the caller's instruction loop.
/// Numeric declines leave through `dispatch`; all other declines fall through
/// to the caller's canonical handler without changing its PC or inputs. Every
/// `?` propagates directly from the caller's running function.
macro_rules! execute {
    (
        $operation:expr, $runtime:expr, $executable:expr, $slots:ident,
        $pc:expr, $next_pc:ident, $store_drop:expr;
        finish = $finish:ident, finish_span = $finish_span:ident,
        dispatch = $dispatch:lifetime
    ) => {{
        use $crate::engine::code::quick::tag as Q;
        use $crate::engine::value::JsValue;
        use $crate::engine::vm::numeric::operation::NumericKind as N;
        use $crate::engine::vm::stack::StoreMode;

        let operation = $operation;
        let runtime = $runtime;
        let executable = $executable;
        let pc = $pc;
        match operation.tag() {
            Q::GENERIC_CANONICAL => {}
            Q::NOP => $finish!(),
            Q::PUSH_I32 => {
                $slots.push(JsValue::Int(operation.i32_operand()))?;
                $finish!();
            }
            Q::UNDEFINED => {
                $slots.push(JsValue::Undefined)?;
                $finish!();
            }
            Q::NULL => {
                $slots.push(JsValue::Null)?;
                $finish!();
            }
            Q::BOOL => {
                $slots.push(JsValue::Bool(operation.boolean()))?;
                $finish!();
            }
            Q::GOTO => {
                $next_pc = usize::try_from(operation.operand()).map_err(|_| {
                    $crate::engine::vm::run::cold::internal("jump target does not fit PC")
                })?;
                $finish!();
            }
            Q::ADD => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::Add, $finish, $dispatch
            ),
            Q::SUB => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::Sub, $finish, $dispatch
            ),
            Q::MUL => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::Mul, $finish, $dispatch
            ),
            Q::DIV => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::Div, $finish, $dispatch
            ),
            Q::MOD => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::Mod, $finish, $dispatch
            ),
            Q::POW => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::Pow, $finish, $dispatch
            ),
            Q::SHL => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::Shl, $finish, $dispatch
            ),
            Q::SAR => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::Sar, $finish, $dispatch
            ),
            Q::SHR => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::Shr, $finish, $dispatch
            ),
            Q::BIT_AND => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::BitAnd, $finish, $dispatch
            ),
            Q::BIT_OR => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::BitOr, $finish, $dispatch
            ),
            Q::BIT_XOR => $crate::engine::vm::run::quick::execute!(
                @binary $slots, N::BitXor, $finish, $dispatch
            ),
            Q::EQ => $crate::engine::vm::run::quick::execute!(
                @comparison executable, $slots, pc, N::Eq, $next_pc, $finish, $finish_span,
                $dispatch
            ),
            Q::NEQ => $crate::engine::vm::run::quick::execute!(
                @comparison executable, $slots, pc, N::Neq, $next_pc, $finish, $finish_span,
                $dispatch
            ),
            Q::LT => $crate::engine::vm::run::quick::execute!(
                @comparison executable, $slots, pc, N::Lt, $next_pc, $finish, $finish_span,
                $dispatch
            ),
            Q::LTE => $crate::engine::vm::run::quick::execute!(
                @comparison executable, $slots, pc, N::Lte, $next_pc, $finish, $finish_span,
                $dispatch
            ),
            Q::GT => $crate::engine::vm::run::quick::execute!(
                @comparison executable, $slots, pc, N::Gt, $next_pc, $finish, $finish_span,
                $dispatch
            ),
            Q::GTE => $crate::engine::vm::run::quick::execute!(
                @comparison executable, $slots, pc, N::Gte, $next_pc, $finish, $finish_span,
                $dispatch
            ),
            Q::IF_TRUE => $crate::engine::vm::run::quick::execute!(
                @branch $slots, operation.operand(), true, $next_pc, $finish
            ),
            Q::IF_FALSE => $crate::engine::vm::run::quick::execute!(
                @branch $slots, operation.operand(), false, $next_pc, $finish
            ),
            Q::GET_LOCAL => {
                let index = operation.slot();
                if let Some(update) = executable.fusion.update(pc)
                    && $crate::engine::vm::run::fusion::update_local(&mut $slots, index, update)?
                {
                    $next_pc = pc + update.instructions;
                    $finish_span!(update.instructions);
                }
                if executable.fusion.local_add_span(pc).is_none()
                    && $crate::engine::vm::run::hot::read_scalar(runtime, &mut $slots, index, false)?
                {
                    $finish!();
                }
            }
            Q::GET_ARG => {
                if $crate::engine::vm::run::hot::read_scalar(runtime, &mut $slots, operation.slot(), true)? {
                    $finish!();
                }
            }
            Q::PUT_LOCAL => $crate::engine::vm::run::quick::execute!(
                @store runtime, executable, $slots, pc, operation.slot(), false,
                StoreMode::Consume, $next_pc, $store_drop, $finish, $finish_span
            ),
            Q::SET_LOCAL => $crate::engine::vm::run::quick::execute!(
                @store runtime, executable, $slots, pc, operation.slot(), false,
                StoreMode::Keep, $next_pc, $store_drop, $finish, $finish_span
            ),
            Q::PUT_ARG => $crate::engine::vm::run::quick::execute!(
                @store runtime, executable, $slots, pc, operation.slot(), true,
                StoreMode::Consume, $next_pc, $store_drop, $finish, $finish_span
            ),
            Q::SET_ARG => $crate::engine::vm::run::quick::execute!(
                @store runtime, executable, $slots, pc, operation.slot(), true, StoreMode::Keep,
                $next_pc, $store_drop, $finish, $finish_span
            ),
            _ => return Err($crate::engine::vm::run::cold::internal(
                "published QuickOp has an invalid tag"
            )),
        }
    }};
    (@numeric $kind:expr, $dispatch:lifetime) => {{
        #[cfg(feature = "profiling")]
        $crate::engine::vm::run::cold::event("quick.dispatch.numeric");
        break $dispatch Some($kind);
    }};
    (@binary $slots:ident, $kind:expr, $finish:ident, $dispatch:lifetime) => {{
        if $crate::engine::vm::run::hot::binary(&mut $slots, $kind)? {
            $finish!();
        } else {
            $crate::engine::vm::run::quick::execute!(@numeric $kind, $dispatch);
        }
    }};
    (
        @comparison $executable:ident, $slots:ident, $pc:ident, $kind:expr,
        $next_pc:ident, $finish:ident, $finish_span:ident, $dispatch:lifetime
    ) => {{
        if $executable.fusion.compare_branch($pc) {
            #[cfg(feature = "profiling")]
            {
                $crate::engine::vm::run::cold::event("quick.span_canonical_fetch");
                $crate::engine::vm::run::cold::event("quick.span_canonical_fetch");
            }
            if let Some(target) = $crate::engine::vm::run::fusion::compare_branch(
                &mut $slots, &$executable.code[$pc], &$executable.code[$pc + 1],
            )? {
                $next_pc = if target == usize::MAX { $pc + 2 } else { target };
                $finish_span!(2_usize);
            } else {
                // The span already proved the pair is not Number without
                // consuming it. Carry the kind directly; never re-probe it.
                $crate::engine::vm::run::quick::execute!(@numeric $kind, $dispatch);
            }
        } else {
            $crate::engine::vm::run::quick::execute!(@binary $slots, $kind, $finish, $dispatch);
        }
    }};
    (@branch $slots:ident, $target:expr, $when:expr, $next_pc:ident, $finish:ident) => {{
        if let Some(target) = $crate::engine::vm::run::hot::branch(&mut $slots, $target, $when)? {
            if target != usize::MAX {
                $next_pc = target;
            }
            $finish!();
        }
    }};
    (
        @store $runtime:ident, $executable:ident, $slots:ident, $pc:ident,
        $index:expr, $argument:expr, $mode:expr, $next_pc:ident, $store_drop:expr,
        $finish:ident, $finish_span:ident
    ) => {{
        let index = $index;
        let argument = $argument;
        let mode = $mode;
        'quick_store: {
            #[cfg(any(test, oxide_store_drop_fusion))]
            if $store_drop
                && matches!(mode, $crate::engine::vm::stack::StoreMode::Keep)
                && let Some(kind) = $executable.fusion.store_drop($pc)
            {
                #[cfg(feature = "profiling")]
                $crate::engine::vm::run::cold::event("quick.span_canonical_fetch");
                let result = $crate::engine::vm::run::fusion::store_drop(
                    $runtime, &mut $slots, kind, &$executable.code[$pc],
                );
                if matches!(result, Ok(false)) {
                    // The canonical handler owns the decline and its counters.
                    // Do not probe a second scalar store here.
                    break 'quick_store;
                }
                #[cfg(feature = "profiling")]
                $crate::engine::vm::run::cold::event("fusion.StoreDropCandidate");
                result?;
                $next_pc = $pc + 2;
                $finish_span!(2_usize);
            }
            if !argument
                && !$executable.local_definitions.get(usize::from(index)).is_some_and(|definition| {
                    definition.kind == $crate::engine::code::function::metadata::ClosureVariableKind::Normal
                        && !definition.is_const
                })
            {
                break 'quick_store;
            }
            if $crate::engine::vm::run::hot::store_scalar($runtime, &mut $slots, index, argument, mode)? {
                $finish!();
            }
        }
    }};
}

pub(super) use execute;
