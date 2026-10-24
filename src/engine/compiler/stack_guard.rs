//! Parser recursion budgeting with a physical stack backstop.
//!
//! QuickJS bounds parser recursion by checking its C stack in `next_token()`
//! against the runtime's one-MiB stack budget. Rust parser frames differ from
//! QuickJS's C frames, so the parser charges each recursive grammar edge a
//! weight against a logical budget instead. The weights approximate the
//! nesting depths at which QuickJS throws; QuickJS's own depths shift by a few
//! percent with the C compiler and flags it is built with. Every committed
//! token advance also samples the native stack, which protects productions
//! that carry no weight of their own.

use std::cell::Cell;

/// Logical parser budget. A frame of weight `w` nests `PARSER_STACK_BUDGET / w`
/// times before the parser reports `SyntaxError: stack overflow`.
const PARSER_STACK_BUDGET: u64 = 1 << 32;

/// Maximum native-stack reserve kept below the deepest token-edge check. A
/// parser which starts near the bottom of a small stack uses the adaptive
/// fraction in [`ParserStackGuard::new`] so shallow source remains usable.
const PARSER_STACK_RESERVE: usize = if cfg!(debug_assertions) {
    96 * 1024
} else {
    16 * 1024
};

/// Never descend without enough room to construct and unwind the syntax
/// error. Token-edge sampling is frequent enough for this lower allowance on
/// small stacks; the regression test exercises a 2 MiB thread.
const PARSER_STACK_MIN_RESERVE: usize = if cfg!(debug_assertions) {
    24 * 1024
} else {
    8 * 1024
};

/// Native stack a parse may consume from its entry point on a thread whose
/// extent is unknown. Rust spawns 2 MiB threads by default, and the runtime's
/// own call budget is one MiB, so this stays within such a thread. A
/// WebAssembly module's whole stack is one MiB by default. Hosts that run on
/// larger threads declare them with [`declare_thread_stack`].
const PARSER_THREAD_FALLBACK: usize = if cfg!(target_family = "wasm") {
    512 * 1024
} else {
    1024 * 1024
};

/// Native stack a parse may consume on a main thread whose RLIMIT_STACK is
/// `unlimited`. The growable `[stack]` mapping has no known floor then; this
/// stays below the common 8 MiB main-thread stack.
#[cfg(target_os = "linux")]
const PARSER_UNLIMITED_MAIN_GROWTH: usize = if cfg!(debug_assertions) {
    1024 * 1024
} else {
    6 * 1024 * 1024
};

/// Frames above a [`declare_thread_stack`] call that the declared size still
/// covers: the thread entry trampoline, the host closure and thread-local
/// storage that the C library places in the stack mapping.
const DECLARED_STACK_SLACK: usize = 128 * 1024;

thread_local! {
    /// Lowest usable stack address declared by the host for this thread.
    static DECLARED_STACK_FLOOR: Cell<Option<usize>> = const { Cell::new(None) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(target_os = "linux")]
struct StackRegion {
    low: usize,
    high: usize,
    growable_main: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg(target_os = "linux")]
struct LinuxStackMetadata {
    region: Option<StackRegion>,
    rlimit_soft: Option<usize>,
}

#[cfg(target_os = "linux")]
thread_local! {
    /// /proc describes the current thread's stack mapping. Read it once per
    /// thread rather than once per Parser: eval, Function and module
    /// compilation can construct thousands of parsers on the same thread.
    static LINUX_STACK_METADATA: Cell<Option<LinuxStackMetadata>> = const { Cell::new(None) };
    #[cfg(test)]
    static LINUX_STACK_METADATA_LOADS: Cell<usize> = const { Cell::new(0) };
}

/// Marginal logical cost of one recursive grammar edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParserStackFrame {
    Parenthesized,
    ArrayLiteral,
    ObjectLiteral,
    CallArguments,
    /// Additional cost of a construct argument list. It composes with
    /// NewWithoutArguments so nested `new F(...)` matches an ordinary call.
    ConstructArguments,
    Unary,
    Conditional,
    Arrow,
    ArrowBlockBody,
    Template,
    StatementHead,
    Block,
    FunctionBody,
    MemberAccess,
    Assignment,
    Exponentiation,
    Yield,
    DynamicImport,
    Label,
    NewWithoutArguments,
}

impl ParserStackFrame {
    /// Each weight is `PARSER_STACK_BUDGET / (depth - 1)` for the depth at
    /// which a QuickJS 2026-06-04 reference build first throws when the form
    /// nests inside `eval`; the unit test below lists those depths.
    const fn weight(self) -> u64 {
        match self {
            Self::Parenthesized => 5_990_191,
            Self::ArrayLiteral | Self::CallArguments => 5_788_365,
            Self::ObjectLiteral => 6_114_011,
            Self::ConstructArguments => 5_064_576,
            Self::Unary | Self::Exponentiation => 460_388,
            Self::Conditional | Self::Assignment => 526_150,
            Self::Arrow => 920_876,
            Self::ArrowBlockBody => 2_105_880,
            Self::Template => 6_774_396,
            Self::StatementHead => 1_249_264,
            Self::Block => 1_315_055,
            Self::FunctionBody => 1_644_321,
            Self::MemberAccess => 5_924_092,
            Self::Yield => 526_175,
            Self::DynamicImport => 5_787_000,
            Self::Label => 1_249_626,
            Self::NewWithoutArguments => 723_500,
        }
    }
}

#[inline(never)]
fn current_stack_address() -> usize {
    let marker = 0_u8;
    std::ptr::from_ref(&marker).addr()
}

/// Declare that the current thread's stack extends `size` bytes below the
/// thread's entry point. Call this first thing on a thread the host spawned
/// with that stack size; the parser then uses the whole stack instead of its
/// conservative default for threads of unknown size.
pub(crate) fn declare_thread_stack(size: usize) {
    let floor = current_stack_address()
        .saturating_sub(size)
        .saturating_add(DECLARED_STACK_SLACK);
    DECLARED_STACK_FLOOR.with(|declared| declared.set(Some(floor)));
}

#[cfg(target_os = "linux")]
fn parse_linux_stack_metadata(entry: usize, maps: &str, limits: &str) -> LinuxStackMetadata {
    let rlimit_soft = limits
        .lines()
        .find(|line| line.starts_with("Max stack size"))
        .and_then(|line| line.split_whitespace().nth(3))
        // 'unlimited' is intentionally None: a growable main-stack VMA's
        // current low edge is not a real floor in that configuration.
        .and_then(|soft| soft.parse::<usize>().ok());
    let region = maps.lines().find_map(|line| {
        let range = line.split_whitespace().next()?;
        let (low, high) = range.split_once('-')?;
        let low = usize::from_str_radix(low, 16).ok()?;
        let high = usize::from_str_radix(high, 16).ok()?;
        (entry >= low && entry < high).then_some(StackRegion {
            low,
            high,
            growable_main: line.split_whitespace().last() == Some("[stack]"),
        })
    });
    LinuxStackMetadata {
        region,
        rlimit_soft,
    }
}

#[cfg(target_os = "linux")]
fn linux_stack_metadata(entry: usize) -> LinuxStackMetadata {
    LINUX_STACK_METADATA.with(|cached| {
        if let Some(metadata) = cached.get() {
            return metadata;
        }
        #[cfg(test)]
        LINUX_STACK_METADATA_LOADS.with(|loads| loads.update(|value| value + 1));
        let metadata = match (
            std::fs::read_to_string("/proc/self/maps"),
            std::fs::read_to_string("/proc/self/limits"),
        ) {
            (Ok(maps), Ok(limits)) => parse_linux_stack_metadata(entry, &maps, &limits),
            _ => LinuxStackMetadata::default(),
        };
        cached.set(Some(metadata));
        metadata
    })
}

#[cfg(target_os = "linux")]
fn metadata_floor(entry: usize, metadata: LinuxStackMetadata) -> usize {
    let thread_fallback = entry.saturating_sub(PARSER_THREAD_FALLBACK);
    let Some(region) = metadata.region else {
        return thread_fallback;
    };
    if region.growable_main {
        // A main thread's `[stack]` VMA grows downward as the program touches
        // deeper frames, so its cached low edge is only where the mapping
        // ended when `/proc/self/maps` was first read. The fixed high edge
        // and the RLIMIT_STACK extent describe the real floor; `unlimited`
        // has none, so the parse gets a bounded growth from its entry.
        return metadata.rlimit_soft.map_or_else(
            || entry.saturating_sub(PARSER_UNLIMITED_MAIN_GROWTH),
            |limit| region.high.saturating_sub(limit),
        );
    }
    // Any other thread's stack shares its mapping with neighbouring anonymous
    // memory whenever the C library installs guard pages without splitting
    // the mapping, so the mapping's low edge is only a lower bound.
    region.low.max(thread_fallback)
}

#[cfg(target_os = "linux")]
fn stack_floor(entry: usize) -> usize {
    metadata_floor(entry, linux_stack_metadata(entry))
}

#[cfg(not(target_os = "linux"))]
fn stack_floor(entry: usize) -> usize {
    entry.saturating_sub(PARSER_THREAD_FALLBACK)
}

/// Guard pages a thread's stack mapping may include below its usable part.
const STACK_GUARD_ALLOWANCE: usize = 64 * 1024;

/// Host stack below the caller down to this thread's floor, and whether the
/// host declared the thread's size. An undeclared thread's floor is its
/// stack mapping's low edge above a guard allowance (a main thread's from
/// RLIMIT_STACK), else the conservative fallback.
pub(crate) fn available_stack() -> (usize, bool) {
    let entry = current_stack_address();
    if let Some(floor) = DECLARED_STACK_FLOOR
        .with(Cell::get)
        .filter(|floor| *floor < entry)
    {
        return (entry - floor, true);
    }
    #[cfg(target_os = "linux")]
    let floor = {
        let metadata = linux_stack_metadata(entry);
        match metadata.region {
            Some(region) if !region.growable_main => {
                region.low.saturating_add(STACK_GUARD_ALLOWANCE)
            }
            _ => metadata_floor(entry, metadata),
        }
    };
    #[cfg(not(target_os = "linux"))]
    let floor = stack_floor(entry);
    (entry.saturating_sub(floor), false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParserStackOverflow;

pub(crate) struct ParserStackGuard {
    logical: u64,
    floor: usize,
    reserve: usize,
}

impl ParserStackGuard {
    pub(crate) fn new() -> Self {
        let entry = current_stack_address();
        let floor = DECLARED_STACK_FLOOR
            .with(Cell::get)
            .filter(|floor| *floor < entry)
            .unwrap_or_else(|| stack_floor(entry));
        let available = entry.saturating_sub(floor);
        Self {
            logical: 0,
            floor,
            reserve: (available / 10).clamp(PARSER_STACK_MIN_RESERVE, PARSER_STACK_RESERVE),
        }
    }

    /// Check at every committed token edge, as QuickJS's next_token() does.
    /// This is independent of logical weights, so it also covers recursive
    /// productions without one.
    pub(crate) fn check_physical(&self) -> Result<(), ParserStackOverflow> {
        let current = current_stack_address();
        if current <= self.floor.saturating_add(self.reserve) {
            Err(ParserStackOverflow)
        } else {
            Ok(())
        }
    }

    pub(crate) fn enter(&mut self, frame: ParserStackFrame) -> Result<u64, ParserStackOverflow> {
        self.check_physical()?;
        let weight = frame.weight();
        if self.logical.saturating_add(weight) > PARSER_STACK_BUDGET {
            return Err(ParserStackOverflow);
        }
        self.logical += weight;
        Ok(weight)
    }

    pub(crate) fn leave(&mut self, weight: u64) {
        self.logical = self.logical.saturating_sub(weight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_throw_depth(weight: u64) -> u64 {
        PARSER_STACK_BUDGET / weight + 1
    }

    #[test]
    fn weights_reproduce_the_reference_depths() {
        for (frames, depth) in [
            (&[ParserStackFrame::Parenthesized][..], 718),
            (&[ParserStackFrame::ArrayLiteral], 743),
            (&[ParserStackFrame::CallArguments], 743),
            (
                &[
                    ParserStackFrame::NewWithoutArguments,
                    ParserStackFrame::ConstructArguments,
                ],
                743,
            ),
            (
                &[
                    ParserStackFrame::Parenthesized,
                    ParserStackFrame::ObjectLiteral,
                ],
                355,
            ),
            (&[ParserStackFrame::Unary], 9330),
            (&[ParserStackFrame::Exponentiation], 9330),
            (&[ParserStackFrame::Conditional], 8164),
            (&[ParserStackFrame::Assignment], 8164),
            (&[ParserStackFrame::Arrow], 4665),
            (
                &[ParserStackFrame::Arrow, ParserStackFrame::ArrowBlockBody],
                1420,
            ),
            (&[ParserStackFrame::Template], 635),
            (&[ParserStackFrame::StatementHead], 3438),
            (&[ParserStackFrame::Label], 3438),
            (&[ParserStackFrame::Block], 3266),
            (&[ParserStackFrame::FunctionBody], 2613),
            (&[ParserStackFrame::MemberAccess], 726),
            (&[ParserStackFrame::NewWithoutArguments], 5937),
        ] {
            let weight = frames.iter().map(|frame| frame.weight()).sum();
            assert_eq!(first_throw_depth(weight), depth, "{frames:?}");
        }
        // Yield and dynamic import nest inside one function body, whose charge
        // stays live for the whole chain.
        let enclosing = ParserStackFrame::FunctionBody.weight();
        for (frame, depth) in [
            (ParserStackFrame::Yield, 8160),
            (ParserStackFrame::DynamicImport, 742),
        ] {
            let weight = frame.weight();
            assert!((depth - 1) * weight + enclosing <= PARSER_STACK_BUDGET);
            assert!(depth * weight + enclosing > PARSER_STACK_BUDGET);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unlimited_main_stack_is_classified_as_unknown() {
        let entry = 0x7fff_f000_usize;
        let metadata = parse_linux_stack_metadata(
            entry,
            "7fff0000-80000000 rw-p 0 0 0 [stack]\n",
            "Max stack size            unlimited            unlimited            bytes\n",
        );
        assert_eq!(metadata.rlimit_soft, None);
        assert!(metadata.region.unwrap().growable_main);
        assert_eq!(
            metadata_floor(entry, metadata),
            entry - PARSER_UNLIMITED_MAIN_GROWTH
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn thread_mappings_bound_the_floor_without_extending_it() {
        let entry = 0x7000_0000_usize;
        // A thread stack merged with neighbouring anonymous memory reports a
        // mapping far larger than the stack itself.
        let merged = parse_linux_stack_metadata(
            entry,
            "6000000-70100000 rw-p 0 0 0\n",
            "Max stack size            8388608              unlimited            bytes\n",
        );
        assert_eq!(
            metadata_floor(entry, merged),
            entry - PARSER_THREAD_FALLBACK
        );
        // A mapping smaller than the fallback is the tighter bound.
        let small = parse_linux_stack_metadata(
            entry,
            "6ffc0000-70010000 rw-p 0 0 0\n",
            "Max stack size            8388608              unlimited            bytes\n",
        );
        assert_eq!(metadata_floor(entry, small), 0x6ffc_0000);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_stack_metadata_is_loaded_once_per_thread() {
        std::thread::spawn(|| {
            LINUX_STACK_METADATA.with(|cached| cached.set(None));
            LINUX_STACK_METADATA_LOADS.with(|loads| loads.set(0));
            for _ in 0..8 {
                let _guard = ParserStackGuard::new();
            }
            LINUX_STACK_METADATA_LOADS.with(|loads| assert_eq!(loads.get(), 1));
        })
        .join()
        .unwrap();
    }

    #[test]
    fn declared_thread_stack_sets_the_floor() {
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                declare_thread_stack(8 * 1024 * 1024);
                let guard = ParserStackGuard::new();
                let entry = current_stack_address();
                assert!(entry - guard.floor > 7 * 1024 * 1024);
                assert!(entry - guard.floor < 8 * 1024 * 1024);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn physical_backstop_stops_recursion_before_host_stack_overflow() {
        use crate::engine::compiler::parser::context::Parser;
        use crate::engine::value::JsString;

        // Nested assignment patterns have no logical weight, so this depends
        // on the token-edge physical check: without it the 2 MiB thread
        // overflows.
        let source = format!("var a;{}a{}", "[".repeat(10_000), "]=[]".repeat(10_000));
        std::thread::Builder::new()
            .name("parser-physical-backstop".to_owned())
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                let error =
                    Parser::parse(&source, JsString::from_static("<parser-physical-backstop>"))
                        .expect_err("deep source must reach the parser stack guard");
                assert_eq!(error.kind(), crate::engine::api::error::ErrorKind::Syntax);
                assert_eq!(error.message(), "stack overflow");
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
