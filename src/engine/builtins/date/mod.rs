//! Pinned QuickJS 2026-06-04 `Date` intrinsic.
//!
//! Calendar, parsing, formatting, and host policy stay in narrow modules;
//! constructor and prototype handlers meet at one typed native-dispatch edge.

mod calendar;
mod constructor;
mod format;
mod parse;
mod prototype;

use super::*;
use crate::engine::object::builtin_properties::{
    BuiltinEntry, BuiltinKey, alias, method, symbol_method,
};

/// Side-effect-free ISO rendering used by the qjs diagnostic value printer.
/// Invalid dates deliberately return `None`: pinned `JS_PrintValue` then falls
/// back to the ordinary `Date { ... }` object shell instead of throwing.
pub(crate) fn qjs_print_iso_string(value: f64) -> Option<String> {
    let fields = calendar::get_date_fields(value, false, false, |_| 0);
    format::format_date_string(
        fields.as_ref(),
        crate::engine::builtins::native::DateStringMethod::IsoString,
    )
    .ok()
}

impl Runtime {
    /// Install the complete pinned `js_date_funcs` and
    /// `js_date_proto_funcs` tables. `%Date.prototype%` is an ordinary object
    /// without a Date payload, exactly as in upstream.
    pub(crate) fn initialize_date_intrinsic(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        date_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        use DateGetFieldKind as Get;
        use DateNativeKind as K;
        use DateSetFieldKind as Set;
        use DateStringMethod as Str;
        const fn date(kind: K, name: &'static str) -> BuiltinEntry {
            method(
                NativeFunctionId::Date(kind),
                name,
                kind.length(),
                kind.length(),
            )
        }
        const fn get(kind: Get) -> BuiltinEntry {
            date(K::GetField(kind), kind.name())
        }
        const fn set(kind: Set) -> BuiltinEntry {
            date(K::SetField(kind), kind.name())
        }
        // Source-table order matters even though OrdinaryOwnPropertyKeys later
        // groups the symbol after all String keys. QuickJS explicitly
        // materializes aliases instead of putting an AutoInit alias in the
        // shape, preserving both identity and the original function `name`.
        const PROTOTYPE: &[BuiltinEntry] = &[
            date(K::TimeValue, "valueOf"),
            date(K::String(Str::String), "toString"),
            symbol_method(
                WellKnownSymbol::ToPrimitive,
                NativeFunctionId::Date(K::ToPrimitive),
                "[Symbol.toPrimitive]",
                K::ToPrimitive.length(),
                PropertyFlags::data(false, false, true),
            ),
            date(K::String(Str::UtcString), "toUTCString"),
            alias(
                BuiltinKey::Name("toGMTString"),
                BuiltinKey::Name("toUTCString"),
            ),
            date(K::String(Str::IsoString), "toISOString"),
            date(K::String(Str::DateString), "toDateString"),
            date(K::String(Str::TimeString), "toTimeString"),
            date(K::String(Str::LocaleString), "toLocaleString"),
            date(K::String(Str::LocaleDateString), "toLocaleDateString"),
            date(K::String(Str::LocaleTimeString), "toLocaleTimeString"),
            date(K::TimezoneOffset, "getTimezoneOffset"),
            date(K::TimeValue, "getTime"),
            get(Get::Year),
            get(Get::FullYear),
            get(Get::UtcFullYear),
            get(Get::Month),
            get(Get::UtcMonth),
            get(Get::Date),
            get(Get::UtcDate),
            get(Get::Hours),
            get(Get::UtcHours),
            get(Get::Minutes),
            get(Get::UtcMinutes),
            get(Get::Seconds),
            get(Get::UtcSeconds),
            get(Get::Milliseconds),
            get(Get::UtcMilliseconds),
            get(Get::Day),
            get(Get::UtcDay),
            date(K::SetTime, "setTime"),
            set(Set::Milliseconds),
            set(Set::UtcMilliseconds),
            set(Set::Seconds),
            set(Set::UtcSeconds),
            set(Set::Minutes),
            set(Set::UtcMinutes),
            set(Set::Hours),
            set(Set::UtcHours),
            set(Set::Date),
            set(Set::UtcDate),
            set(Set::Month),
            set(Set::UtcMonth),
            date(K::SetYear, "setYear"),
            set(Set::FullYear),
            set(Set::UtcFullYear),
            date(K::ToJson, "toJSON"),
        ];
        const CONSTRUCTOR: &[BuiltinEntry] = &[
            date(K::Now, "now"),
            date(K::Parse, "parse"),
            date(K::Utc, "UTC"),
        ];
        self.define_builtin_entries(date_prototype, realm, PROTOTYPE)?;
        let length = K::Constructor.length();
        let constructor = self.new_native_builtin(
            function_prototype,
            realm,
            NativeFunctionId::Date(K::Constructor),
            length,
            "Date",
            i32::from(length),
        )?;
        self.define_builtin_entries(constructor.as_object(), realm, CONSTRUCTOR)?;
        self.define_global_function(global_object, "Date", &constructor)?;
        self.define_constructor_relationship(&constructor, date_prototype)
    }

    pub(crate) fn call_date_native(
        &self,
        realm: ContextId,
        kind: DateNativeKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| match kind {
            DateNativeKind::Constructor
            | DateNativeKind::Now
            | DateNativeKind::Parse
            | DateNativeKind::Utc => {
                self.call_date_constructor_native(realm, kind, invocation, arguments)
            }
            _ => self.call_date_prototype_native(realm, kind, invocation, arguments),
        })
    }

    pub(crate) fn date_now_millis(&self) -> i64 {
        self.0.host_services.now_millis()
    }

    pub(crate) fn date_timezone_offset_minutes(&self, epoch_millis: i64) -> i32 {
        self.0.host_services.timezone_offset_minutes(epoch_millis)
    }
}

pub(crate) use constructor::operation::{DateConstructorResume, DateConstructorStep};

pub(crate) use prototype::operation::{DatePrototypeResume, DatePrototypeStep};

#[cfg(test)]
mod owned_invocation_tests;

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::*;

    #[derive(Debug)]
    struct FixedHostServices {
        now_millis: i64,
        timezone_offset_minutes: i32,
        random_seed: u64,
        timezone_queries: Rc<RefCell<Vec<i64>>>,
        random_seed_calls: Rc<Cell<usize>>,
    }

    impl HostServices for FixedHostServices {
        fn now_millis(&self) -> i64 {
            self.now_millis
        }

        fn timezone_offset_minutes(&self, epoch_millis: i64) -> i32 {
            self.timezone_queries.borrow_mut().push(epoch_millis);
            self.timezone_offset_minutes
        }

        fn random_seed(&self) -> u64 {
            self.random_seed_calls.update(|value| value + 1);
            self.random_seed
        }
    }

    #[test]
    fn runtime_owns_injectable_clock_timezone_and_random_seed_services() {
        let timezone_queries = Rc::new(RefCell::new(Vec::new()));
        let random_seed_calls = Rc::new(Cell::new(0));
        let runtime = Runtime::new_with_host_services(FixedHostServices {
            now_millis: 42,
            timezone_offset_minutes: -480,
            random_seed: 0x1234,
            timezone_queries: timezone_queries.clone(),
            random_seed_calls: random_seed_calls.clone(),
        });
        let mut context = runtime.new_context().expect("create context");

        assert_eq!(context.eval("Date.now()").unwrap(), Value::Int(42));
        assert_eq!(
            context.eval("new Date(1234).getTimezoneOffset()").unwrap(),
            Value::Int(-480)
        );
        assert_eq!(&*timezone_queries.borrow(), &[1234]);
        assert_eq!(random_seed_calls.get(), 1);

        let first_random = context.eval("Math.random()").unwrap();
        let same_seed_runtime = Runtime::new_with_host_services(FixedHostServices {
            now_millis: 0,
            timezone_offset_minutes: 0,
            random_seed: 0x1234,
            timezone_queries: Rc::new(RefCell::new(Vec::new())),
            random_seed_calls: Rc::new(Cell::new(0)),
        });
        let same_random = same_seed_runtime
            .new_context()
            .expect("create context")
            .eval("Math.random()")
            .unwrap();
        assert_eq!(first_random, same_random);

        let different_seed_runtime = Runtime::new_with_host_services(FixedHostServices {
            now_millis: 0,
            timezone_offset_minutes: 0,
            random_seed: 0x1235,
            timezone_queries: Rc::new(RefCell::new(Vec::new())),
            random_seed_calls: Rc::new(Cell::new(0)),
        });
        let different_random = different_seed_runtime
            .new_context()
            .expect("create context")
            .eval("Math.random()")
            .unwrap();
        assert_ne!(first_random, different_random);
    }
}
