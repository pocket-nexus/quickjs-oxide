//! Deterministic parser recursion budgeting with a physical stack backstop.
//!
//! Pinned QuickJS checks its C stack in next_token() (quickjs.c:22719). The
//! Rust parser keeps that structural invariant: every committed token advance
//! samples the native stack, including productions which have no logical
//! weight of their own. A separate weighted budget reproduces the pinned
//! nesting boundaries despite different Rust and C frame sizes.

#[cfg(target_os = "linux")]
use std::cell::Cell;

/// Logical budget used for an eval-created parser. The weights below are
/// calibrated against QuickJS 2026-06-04's default one-MiB stack budget.
const PARSER_STACK_BUDGET: u64 = 1 << 32;

/// Main Script/Module roots start closer to QuickJS's runtime stack top than
/// an eval compiler does. Separate weights below reproduce that observable
/// direct-source boundary; the budget itself remains shared and composable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ParserStackContext {
    Direct,
    Eval,
}

/// Maximum native-stack reserve after the deepest token-edge check.
/// Normal-sized stacks retain the allowance calibrated by task B8. A parser
/// which itself starts near the bottom of a deliberately constrained stack
/// uses the adaptive fraction below so shallow source remains usable.
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

/// A parse may consume at most this much native stack from its entry point
/// when the OS stack extent is unknown. This is deliberately below the 8 MiB
/// main-thread and CLI worker stacks, and it also handles Linux's
/// RLIMIT_STACK=unlimited case without treating the current growable VMA as
/// the real stack floor.
const PARSER_FALLBACK_GROWTH: usize = if cfg!(debug_assertions) {
    1024 * 1024
} else {
    6 * 1024 * 1024
};

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
    /// thread, not once per Parser (eval, Function, and module compilation can
    /// construct thousands of parsers on the same thread).
    static LINUX_STACK_METADATA: Cell<Option<LinuxStackMetadata>> = const { Cell::new(None) };
    #[cfg(test)]
    static LINUX_STACK_METADATA_LOADS: Cell<usize> = const { Cell::new(0) };
}

/// Marginal logical cost of one recursive grammar edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ParserStackFrame {
    Parenthesized,
    ArrayLiteral,
    ObjectLiteral,
    CallArguments,
    /// Additional cost of a construct argument list. It composes with
    /// NewWithoutArguments so nested new F(...) matches an ordinary call.
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
    /// Object literal used as the immediate discriminant of a `with (head)`.
    /// In pinned QuickJS this transient occupies a smaller C frame than an
    /// ordinary object literal; modelling it separately keeps the nested
    /// `with({}){` boundary at the pinned depth instead of charging it like
    /// `({...})`.
    WithObjectHead,
    /// Array literal reached as the immediate operand of a spread element
    /// (`[...[...]]`). Pinned's frame for this chain is marginally smaller
    /// than an ordinary array literal, which admits one extra nesting level.
    SpreadElement,
}

impl ParserStackFrame {
    const fn eval_weight(self) -> u64 {
        match self {
            Self::Parenthesized => 5_990_191,
            Self::ArrayLiteral => 5_788_365,
            Self::ObjectLiteral => 6_114_011,
            Self::CallArguments => 5_788_365,
            Self::ConstructArguments => 5_064_576,
            Self::Unary => 460_388,
            Self::Conditional => 526_150,
            Self::Arrow => 920_876,
            Self::ArrowBlockBody => 2_105_880,
            Self::Template => 6_774_396,
            Self::StatementHead => 1_249_264,
            Self::Block => 1_315_055,
            Self::FunctionBody => 1_644_321,
            Self::MemberAccess => 5_924_092,
            Self::Assignment => 526_150,
            Self::Exponentiation => 460_388,
            Self::Yield => 526_175,
            Self::DynamicImport => 5_787_000,
            Self::Label => 1_249_626,
            Self::NewWithoutArguments => 723_500,
            // With SH charged at the body token, the object-head transient
            // (1674 surrounding statement/block pairs live) must stay below
            // 2,297,290 in eval so the 1675th head fits and the body block
            // trips first, matching pinned's body-token column.
            Self::WithObjectHead => 2_200_000,
            // `[...[...[]]]` admits one more nesting (pinned throw 743) than
            // the generic array weight (742).
            Self::SpreadElement => 5_776_674,
        }
    }

    /// Direct source is parsed a few C frames nearer QuickJS's recorded stack
    /// top than eval source. Most observed boundaries move by one level; the
    /// statement and block families move by four, so their direct weights are
    /// calibrated independently rather than claiming eval-only exactness.
    const fn direct_weight(self) -> u64 {
        match self {
            Self::Parenthesized => 5_981_848,
            Self::ArrayLiteral | Self::CallArguments => 5_780_575,
            Self::ConstructArguments => 5_056_786,
            Self::Unary | Self::Exponentiation => 459_797,
            Self::Conditional | Self::Assignment => 525_442,
            Self::Arrow => 919_693,
            Self::StatementHead => 1_248_174,
            Self::Block => 1_313_847,
            Self::FunctionBody => 1_642_434,
            Self::MemberAccess => 5_915_932,
            Self::Yield => 525_466,
            Self::DynamicImport => 5_779_000,
            Self::Label => 1_248_000,
            Self::NewWithoutArguments => 722_500,
            // Nested `with({}){` in a direct root: with SH charged at the body
            // token, 1676 statement/block pairs are live when the 1677th head
            // object is entered, leaving 1,020,100 of budget. A weight below
            // that lets the head parse and lets the body block trip first at
            // the pinned 1677 depth/column.
            Self::WithObjectHead => 1_000_000,
            // `[...[...[]]]` direct boundary is 744, one deeper than the
            // generic array weight (743).
            Self::SpreadElement => 5_768_915,
            // A braced arrow body (`x=>{`) starts one frame nearer the budget
            // edge in a direct root, giving the pinned 1422 first-throw depth.
            Self::ArrowBlockBody => 2_101_700,
            Self::ObjectLiteral | Self::Template => self.eval_weight(),
        }
    }

    const fn weight(self, context: ParserStackContext) -> u64 {
        match context {
            ParserStackContext::Direct => self.direct_weight(),
            ParserStackContext::Eval => self.eval_weight(),
        }
    }
}

#[inline(never)]
fn current_stack_address() -> usize {
    let marker = 0_u8;
    std::ptr::from_ref(&marker).addr()
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
        LINUX_STACK_METADATA_LOADS.with(|loads| loads.set(loads.get() + 1));
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
fn stack_floor(entry: usize) -> usize {
    let metadata = linux_stack_metadata(entry);
    let Some(region) = metadata.region else {
        return fallback_floor(entry);
    };
    if region.growable_main {
        // A main thread's `[stack]` VMA grows downward as the program touches
        // deeper frames, so its cached low edge is only where the mapping
        // ended when `/proc/self/maps` was first read — never a real floor.
        // A host which creates its runtime near the top and later evaluates
        // source from a much deeper native callback (host recursion, native
        // getters) legitimately runs below that cached edge. Only the fixed
        // high edge and the RLIMIT_STACK extent describe the real extent;
        // reject the (effectively impossible) entry at or above the high edge.
        if entry < region.high {
            // 'unlimited' (rlimit_soft == None) has no known downward extent,
            // so the entry-relative fallback below applies there too.
            return metadata.rlimit_soft.map_or_else(
                || fallback_floor(entry),
                |limit| region.high.saturating_sub(limit),
            );
        }
    } else if entry >= region.low && entry < region.high {
        // Fixed-size thread stacks (and the main thread on non-Linux hosts)
        // have a stable low edge for the whole thread lifetime.
        return region.low;
    }
    fallback_floor(entry)
}

fn fallback_floor(entry: usize) -> usize {
    entry.saturating_sub(PARSER_FALLBACK_GROWTH)
}

#[cfg(not(target_os = "linux"))]
fn stack_floor(entry: usize) -> usize {
    fallback_floor(entry)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ParserStackOverflow;

pub(super) struct ParserStackGuard {
    logical: u64,
    floor: usize,
    reserve: usize,
    context: ParserStackContext,
}

impl ParserStackGuard {
    pub(super) fn new(context: ParserStackContext) -> Self {
        let entry = current_stack_address();
        let floor = stack_floor(entry);
        let available = entry.saturating_sub(floor);
        Self {
            logical: 0,
            floor,
            reserve: (available / 10).clamp(PARSER_STACK_MIN_RESERVE, PARSER_STACK_RESERVE),
            context,
        }
    }

    /// Check at every committed token edge, mirroring QuickJS next_token().
    /// Keeping this independent of logical weights protects forgotten and
    /// future recursive productions.
    pub(super) fn check_physical(&self) -> Result<(), ParserStackOverflow> {
        let current = current_stack_address();
        if current <= self.floor.saturating_add(self.reserve) {
            Err(ParserStackOverflow)
        } else {
            Ok(())
        }
    }

    pub(super) fn enter(&mut self, frame: ParserStackFrame) -> Result<u64, ParserStackOverflow> {
        self.check_physical()?;
        let weight = frame.weight(self.context);
        if self.logical.saturating_add(weight) > PARSER_STACK_BUDGET {
            return Err(ParserStackOverflow);
        }
        self.logical += weight;
        Ok(weight)
    }

    pub(super) fn leave(&mut self, weight: u64) {
        self.logical = self.logical.saturating_sub(weight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_boundary(context: ParserStackContext, frame: ParserStackFrame, depth: u64) {
        let weight = frame.weight(context);
        assert!(
            (depth - 1) * weight <= PARSER_STACK_BUDGET,
            "{context:?} {frame:?} rejects before {depth} with weight {weight}"
        );
        assert!(
            depth * weight > PARSER_STACK_BUDGET,
            "{context:?} {frame:?} accepts at {depth} with weight {weight}"
        );
    }

    #[test]
    fn weighted_boundaries_match_pinned_quickjs() {
        for (frame, eval_depth, direct_depth) in [
            (ParserStackFrame::Parenthesized, 718, 719),
            (ParserStackFrame::ArrayLiteral, 743, 744),
            (ParserStackFrame::CallArguments, 743, 744),
            (ParserStackFrame::Unary, 9330, 9342),
            (ParserStackFrame::Conditional, 8164, 8175),
            (ParserStackFrame::Arrow, 4665, 4671),
            (ParserStackFrame::Template, 635, 635),
            (ParserStackFrame::StatementHead, 3438, 3442),
            (ParserStackFrame::Block, 3266, 3270),
            (ParserStackFrame::FunctionBody, 2613, 2616),
            (ParserStackFrame::MemberAccess, 726, 727),
            (ParserStackFrame::Assignment, 8164, 8175),
            (ParserStackFrame::Exponentiation, 9330, 9342),
            (ParserStackFrame::Label, 3438, 3442),
            (ParserStackFrame::NewWithoutArguments, 5937, 5945),
        ] {
            assert_boundary(ParserStackContext::Eval, frame, eval_depth);
            assert_boundary(ParserStackContext::Direct, frame, direct_depth);
        }
        for context in [ParserStackContext::Direct, ParserStackContext::Eval] {
            let construct = ParserStackFrame::NewWithoutArguments.weight(context)
                + ParserStackFrame::ConstructArguments.weight(context);
            let depth = match context {
                ParserStackContext::Direct => 744,
                ParserStackContext::Eval => 743,
            };
            assert!((depth - 1) * construct <= PARSER_STACK_BUDGET);
            assert!(depth * construct > PARSER_STACK_BUDGET);
        }
        for (context, depth) in [
            (ParserStackContext::Eval, 355),
            (ParserStackContext::Direct, 356),
        ] {
            let object = ParserStackFrame::Parenthesized.weight(context)
                + ParserStackFrame::ObjectLiteral.weight(context);
            assert!((depth - 1) * object <= PARSER_STACK_BUDGET);
            assert!(depth * object > PARSER_STACK_BUDGET);
        }
        // Braced arrow body (`x=>{`): eval first throws at 1420, a direct
        // Script/Module root starts one frame nearer the budget edge and first
        // throws at 1422.
        for (context, depth) in [
            (ParserStackContext::Eval, 1420),
            (ParserStackContext::Direct, 1422),
        ] {
            let block_arrow = ParserStackFrame::Arrow.weight(context)
                + ParserStackFrame::ArrowBlockBody.weight(context);
            assert!(
                (depth - 1) * block_arrow <= PARSER_STACK_BUDGET,
                "block arrow {context:?} must accept {depth}",
            );
            assert!(
                depth * block_arrow > PARSER_STACK_BUDGET,
                "block arrow {context:?} must throw at {depth}",
            );
        }
        let direct_braced = ParserStackFrame::StatementHead.weight(ParserStackContext::Direct)
            + ParserStackFrame::Block.weight(ParserStackContext::Direct);
        assert!(1676 * direct_braced <= PARSER_STACK_BUDGET);
        assert!(1677 * direct_braced > PARSER_STACK_BUDGET);
        // `with({}){` in a direct root: the smaller transient object-head
        // charge accepts through 1676, then the statement/block frames throw
        // at 1677. Eval keeps the approved 1674 deviation.
        let with_head = ParserStackFrame::WithObjectHead.weight(ParserStackContext::Direct);
        // `with({}){` in a direct root: SH is charged at the body token, so
        // while the 1677th head object is entered only 1676 pairs are live.
        // Its smaller transient charge must fit; the body block then trips at
        // the pinned 1677 depth. Eval follows the same deferred-head shape.
        assert!(1676 * direct_braced + with_head <= PARSER_STACK_BUDGET);
        assert!(1677 * direct_braced > PARSER_STACK_BUDGET);
        // Spread chains `[...[...[]]]`: the outer array is an ordinary array,
        // every nested operand is a spread element. Eval first throws at 743,
        // a direct root at 744.
        for (context, depth, plain, spread) in [
            (ParserStackContext::Eval, 743, 5_788_365, 5_776_674_u64),
            (ParserStackContext::Direct, 744, 5_780_575, 5_768_915_u64),
        ] {
            assert_eq!(
                ParserStackFrame::SpreadElement.weight(context),
                spread,
                "spread element weight {context:?}",
            );
            assert!(plain + (depth - 1) * spread <= PARSER_STACK_BUDGET);
            assert!(plain + depth * spread > PARSER_STACK_BUDGET);
        }
        // These forms occur inside a function body, whose one enclosing
        // logical charge remains live throughout the recursive chain.
        for (frame, context, depth) in [
            (ParserStackFrame::Yield, ParserStackContext::Eval, 8160),
            (ParserStackFrame::Yield, ParserStackContext::Direct, 8171),
            (
                ParserStackFrame::DynamicImport,
                ParserStackContext::Eval,
                742,
            ),
            (
                ParserStackFrame::DynamicImport,
                ParserStackContext::Direct,
                743,
            ),
        ] {
            let weight = frame.weight(context);
            let enclosing = ParserStackFrame::FunctionBody.weight(context);
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
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_stack_metadata_is_loaded_once_per_thread() {
        std::thread::spawn(|| {
            LINUX_STACK_METADATA.with(|cached| cached.set(None));
            LINUX_STACK_METADATA_LOADS.with(|loads| loads.set(0));
            for _ in 0..8 {
                let _guard = ParserStackGuard::new(ParserStackContext::Direct);
            }
            LINUX_STACK_METADATA_LOADS.with(|loads| assert_eq!(loads.get(), 1));
        })
        .join()
        .unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn physical_backstop_stops_recursion_before_host_stack_overflow() {
        use crate::engine::compiler::parser::context::Parser;
        use crate::engine::value::JsString;

        // Nested assignment patterns intentionally have no logical weight.
        // This test therefore depends on the token-edge physical check: a
        // mutation which removes that check overflows this 2 MiB thread.
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
