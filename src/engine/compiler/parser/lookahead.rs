//! Memoized clone-lexer lookahead for non-committing parser probes.
//!
//! Every probe clones the committed lexer and rescans forward. The same
//! `(offset, goal, context)` scan is requested by several probes: a `for` head
//! is scanned once to select the classic form and again to pick `in`/`of`, and
//! a parenthesized parameter list is scanned by the arrow probe, the
//! formal-parameter pre-scan and the bound-name counter. `LookaheadCache`
//! memoizes those scans, so a probe asks for the token at its current lexer
//! position first and only rescans on a miss.
//!
//! Scanning is a pure function of `(source, offset, goal, context)`, so a cache
//! entry never becomes semantically wrong. The committed path consults the same
//! cache before scanning, so a token a probe already scanned is not lexed
//! twice. Invalidation only bounds memory and drops regions the parser has
//! already committed past; the cache is capped and front-compacts so a probe
//! that scans an arbitrarily long region cannot make parsing quadratic.

use crate::engine::compiler::destructuring::{BindingPatternScan, ParenthesizedParameterScan};
use crate::engine::compiler::lexer::LexContext;
use crate::engine::compiler::lexer::LexError;
use crate::engine::compiler::lexer::Lexer;
use crate::engine::compiler::lexer::LexicalGoal;
use crate::engine::compiler::lexer::Punctuator;
use crate::engine::compiler::lexer::Token;
use crate::engine::compiler::parser::context::ForIterationKind;
use crate::engine::compiler::parser::context::Parser;
use std::cell::Cell;
use std::collections::HashMap;

/// Upper bound on memoized entries. A probe that scans a giant region (say a
/// whole array literal looking for its initializer) stops caching past this
/// point and the commit path falls back to scanning, which bounds memory and
/// keeps front invalidation cheap.
const MAX_ENTRIES: usize = 8 * 1024;
/// Compact the invalidated prefix once it is at least this large and no
/// smaller than the live region, keeping `invalidate_before` amortized O(1).
const COMPACT_MIN_PREFIX: usize = 64;
const MAX_PARENTHESIS_SUMMARIES: usize = 1024;

#[derive(Default)]
pub(in crate::engine::compiler) struct LookaheadCache<'source> {
    entries: Vec<LookaheadEntry<'source>>,
    /// `entries[..base]` are invalidated and await compaction.
    base: usize,
    /// Probes and committed parsing usually consume consecutive entries.
    /// Remember the last hit so those scans do not binary-search the entire
    /// window for every token. This is only a hint; every hit checks its key.
    cursor: Cell<usize>,
    /// Complete nested cover-grammar probes can be reused when real parsing
    /// reaches the same parentheses. Failed/depth-limited probes are never
    /// recorded. Context is checked because yield/await affect tokenization.
    parentheses: HashMap<usize, (LexContext, bool)>,
    parentheses_pruned_at: Option<usize>,
    // One result per probe family is enough for adjacent grammar consumers.
    // These bounded summaries are pure functions of offset/context and never
    // evict token entries or grow with source length.
    pub(in crate::engine::compiler) binding_scan: Option<BindingScanMemo<'source>>,
    pub(in crate::engine::compiler) parameter_scan:
        Option<(usize, LexContext, Option<ParenthesizedParameterScan>)>,
    pub(in crate::engine::compiler) iteration_hint:
        Option<(usize, LexContext, Option<ForIterationKind>)>,
}

#[derive(Clone, Copy)]
pub(in crate::engine::compiler) struct BindingScanMemo<'source> {
    pub(in crate::engine::compiler) start: usize,
    pub(in crate::engine::compiler) context: LexContext,
    pub(in crate::engine::compiler) opening: Punctuator,
    pub(in crate::engine::compiler) scan: Option<BindingPatternScan<'source>>,
    pub(in crate::engine::compiler) assignment_seen: bool,
}

struct LookaheadEntry<'source> {
    start: usize,
    goal: LexicalGoal,
    context: LexContext,
    token: Token<'source>,
}

impl<'source> LookaheadCache<'source> {
    fn active(&self) -> &[LookaheadEntry<'source>] {
        &self.entries[self.base..]
    }

    fn peek(&self, start: usize, goal: LexicalGoal, context: LexContext) -> Option<Token<'source>> {
        let key = (start, goal, context);
        for index in [
            self.cursor.get().saturating_add(1),
            self.cursor.get(),
            self.base,
        ] {
            if index < self.base {
                continue;
            }
            if let Some(entry) = self.entries.get(index) {
                if (entry.start, entry.goal, entry.context) == key {
                    self.cursor.set(index);
                    return Some(entry.token);
                }
            }
        }
        let index = self
            .active()
            .binary_search_by(|entry| (entry.start, entry.goal, entry.context).cmp(&key))
            .ok()?;
        self.cursor.set(self.base + index);
        Some(self.active()[index].token)
    }

    fn insert(
        &mut self,
        start: usize,
        goal: LexicalGoal,
        context: LexContext,
        token: Token<'source>,
    ) {
        if self.active().len() >= MAX_ENTRIES {
            return;
        }
        let key = (start, goal, context);
        let index = if self
            .active()
            .last()
            .is_none_or(|entry| (entry.start, entry.goal, entry.context) < key)
        {
            self.active().len()
        } else {
            self.active()
                .partition_point(|entry| (entry.start, entry.goal, entry.context) < key)
        };
        self.entries.insert(
            self.base + index,
            LookaheadEntry {
                start,
                goal,
                context,
                token,
            },
        );
        self.cursor.set(self.base + index);
    }

    /// Drops entries that start at or after `start`, used when a goal or
    /// context change makes that whole suffix stale.
    fn invalidate_from(&mut self, start: usize) {
        let kept = self.active().partition_point(|entry| entry.start < start);
        self.entries.truncate(self.base + kept);
    }

    /// Drops entries that start before `start`, keeping only the region the
    /// parser has not committed past.
    fn invalidate_before(&mut self, start: usize) {
        if self
            .active()
            .first()
            .is_none_or(|entry| entry.start >= start)
        {
            return;
        }
        self.base += self.active().partition_point(|entry| entry.start < start);
        if self.base >= COMPACT_MIN_PREFIX && self.base >= self.entries.len() - self.base {
            self.cursor.set(self.cursor.get().saturating_sub(self.base));
            self.entries.drain(..self.base);
            self.base = 0;
        }
    }

    #[cfg(any(test, feature = "profiling"))]
    fn len(&self) -> usize {
        self.entries.len() - self.base
    }
}

impl<'source> Parser<'source> {
    pub(in crate::engine::compiler) fn cached_parenthesized_arrow(
        &self,
        start: usize,
    ) -> Option<bool> {
        let context = self.lexer.context();
        self.lookahead
            .borrow()
            .parentheses
            .get(&start)
            .filter(|(cached_context, _)| *cached_context == context)
            .map(|(_, arrow)| *arrow)
    }

    pub(in crate::engine::compiler) fn cache_parenthesized_arrow(&self, start: usize, arrow: bool) {
        let mut cache = self.lookahead.borrow_mut();
        if cache.parentheses.len() >= MAX_PARENTHESIS_SUMMARIES {
            let current = self.current().span.start.byte_offset;
            if cache.parentheses_pruned_at != Some(current) {
                cache.parentheses.retain(|offset, _| *offset >= current);
                cache.parentheses_pruned_at = Some(current);
            }
            if cache.parentheses.len() >= MAX_PARENTHESIS_SUMMARIES {
                return;
            }
        }
        cache
            .parentheses
            .insert(start, (self.lexer.context(), arrow));
    }

    /// One memoized probe step: scan `goal` from the probe lexer's current
    /// position, reusing a memoized token when possible. A hit repositions the
    /// lexer at the token end, so the caller observes the same state a miss
    /// would leave behind.
    pub(in crate::engine::compiler) fn probe_token(
        &self,
        lexer: &mut Lexer<'source>,
        goal: LexicalGoal,
    ) -> Result<Token<'source>, LexError> {
        let context = lexer.context();
        let start = lexer.current_position().byte_offset;
        if let Some(token) = self.lookahead_peek(start, goal, context) {
            lexer.seek(token.span.end);
            return Ok(token);
        }
        let token = lexer.next_token_with_goal(goal)?;
        self.lookahead
            .borrow_mut()
            .insert(start, goal, context, token);
        #[cfg(feature = "profiling")]
        counters::record_entries(self.lookahead.borrow().len());
        Ok(token)
    }

    fn lookahead_peek(
        &self,
        start: usize,
        goal: LexicalGoal,
        context: LexContext,
    ) -> Option<Token<'source>> {
        let token = self.lookahead.borrow().peek(start, goal, context);
        #[cfg(feature = "profiling")]
        counters::record(token.is_some());
        token
    }

    /// Drops memoized scans the parser has already committed past.
    pub(in crate::engine::compiler) fn lookahead_invalidate_before(&self, start: usize) {
        self.lookahead.borrow_mut().invalidate_before(start);
    }

    /// Drops memoized scans invalidated by a goal or context change.
    pub(in crate::engine::compiler) fn lookahead_invalidate_from(&self, start: usize) {
        self.lookahead.borrow_mut().invalidate_from(start);
    }

    /// A commit-path scan consumes a token a probe already memoized. Scanning
    /// is a pure function of the key, so the committed token is byte-identical
    /// to a fresh scan; the lexer is repositioned by the caller.
    pub(in crate::engine::compiler) fn take_lookahead(
        &self,
        start: usize,
        goal: LexicalGoal,
        context: LexContext,
    ) -> Option<Token<'source>> {
        let token = self.lookahead.borrow().peek(start, goal, context);
        #[cfg(feature = "profiling")]
        if token.is_some() {
            counters::record_commit_hit();
        }
        token
    }

    #[cfg(test)]
    pub(in crate::engine::compiler) fn lookahead_entry_count(&self) -> usize {
        self.lookahead.borrow().len()
    }

    #[cfg(test)]
    pub(in crate::engine::compiler) fn lookahead_insert(
        &self,
        start: usize,
        goal: LexicalGoal,
        context: LexContext,
        token: Token<'source>,
    ) {
        self.lookahead
            .borrow_mut()
            .insert(start, goal, context, token);
    }
}

/// Diagnostic probe-cache counters for `feature = "profiling"` builds. The
/// public compile probe prints them; production builds compile them out.
#[cfg(feature = "profiling")]
pub(crate) mod counters {
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;

    static HITS: AtomicU64 = AtomicU64::new(0);
    static MISSES: AtomicU64 = AtomicU64::new(0);
    static COMMIT_HITS: AtomicU64 = AtomicU64::new(0);
    static MAX_ENTRIES: AtomicU64 = AtomicU64::new(0);

    pub(in crate::engine::compiler) fn record(hit: bool) {
        let counter = if hit { &HITS } else { &MISSES };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub(in crate::engine::compiler) fn record_commit_hit() {
        COMMIT_HITS.fetch_add(1, Ordering::Relaxed);
    }

    pub(in crate::engine::compiler) fn record_entries(len: usize) {
        MAX_ENTRIES.fetch_max(len as u64, Ordering::Relaxed);
    }

    pub(crate) fn snapshot() -> (u64, u64, u64, u64) {
        (
            HITS.load(Ordering::Relaxed),
            MISSES.load(Ordering::Relaxed),
            COMMIT_HITS.load(Ordering::Relaxed),
            MAX_ENTRIES.load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::compiler::lexer::Lexer;
    use crate::engine::compiler::lexer::LexicalGoal;

    #[test]
    fn cache_orders_entries_and_scopes_invalidation() {
        let mut lexer = Lexer::new("alpha beta gamma");
        let alpha = lexer.next_token().unwrap();
        let beta = lexer.next_token().unwrap();
        let gamma = lexer.next_token().unwrap();
        let context = LexContext::default();
        let mut cache = LookaheadCache::default();
        cache.insert(
            alpha.span.start.byte_offset,
            LexicalGoal::Div,
            context,
            alpha,
        );
        cache.insert(beta.span.start.byte_offset, LexicalGoal::Div, context, beta);
        cache.insert(
            gamma.span.start.byte_offset,
            LexicalGoal::Div,
            context,
            gamma,
        );

        assert_eq!(
            cache.peek(alpha.span.start.byte_offset, LexicalGoal::Div, context),
            Some(alpha)
        );
        assert_eq!(
            cache.peek(beta.span.start.byte_offset, LexicalGoal::RegExp, context),
            None
        );

        cache.invalidate_from(beta.span.start.byte_offset);
        assert_eq!(
            cache.peek(alpha.span.start.byte_offset, LexicalGoal::Div, context),
            Some(alpha)
        );
        assert_eq!(
            cache.peek(beta.span.start.byte_offset, LexicalGoal::Div, context),
            None
        );

        cache.insert(beta.span.start.byte_offset, LexicalGoal::Div, context, beta);
        cache.insert(
            gamma.span.start.byte_offset,
            LexicalGoal::Div,
            context,
            gamma,
        );
        cache.invalidate_before(beta.span.start.byte_offset);
        assert_eq!(cache.len(), 2);
        assert_eq!(
            cache.peek(alpha.span.start.byte_offset, LexicalGoal::Div, context),
            None
        );
        assert_eq!(
            cache.peek(beta.span.start.byte_offset, LexicalGoal::Div, context),
            Some(beta)
        );
    }

    #[test]
    fn cache_inserts_keep_offsets_sorted() {
        let mut lexer = Lexer::new("alpha beta gamma");
        let alpha = lexer.next_token().unwrap();
        let beta = lexer.next_token().unwrap();
        let context = LexContext::default();
        let mut cache = LookaheadCache::default();
        cache.insert(beta.span.start.byte_offset, LexicalGoal::Div, context, beta);
        cache.insert(
            alpha.span.start.byte_offset,
            LexicalGoal::Div,
            context,
            alpha,
        );
        assert_eq!(cache.entries[0].start, alpha.span.start.byte_offset);
        assert_eq!(cache.entries[1].start, beta.span.start.byte_offset);
    }

    #[test]
    fn cache_bounds_entries_and_compacts_the_invalidated_prefix() {
        let mut lexer = Lexer::new("alpha beta gamma");
        let beta = lexer.next_token().unwrap();
        let context = LexContext::default();

        let mut cache = LookaheadCache::default();
        for offset in 0..MAX_ENTRIES + 10 {
            cache.insert(1_000_000 + offset, LexicalGoal::Div, context, beta);
        }
        assert_eq!(cache.len(), MAX_ENTRIES);

        let mut cache = LookaheadCache::default();
        for offset in 0..2 * COMPACT_MIN_PREFIX {
            cache.insert(offset, LexicalGoal::Div, context, beta);
        }
        cache.invalidate_before(COMPACT_MIN_PREFIX);
        assert_eq!(cache.base, 0);
        assert_eq!(cache.len(), COMPACT_MIN_PREFIX);
        assert_eq!(cache.peek(0, LexicalGoal::Div, context), None);
        assert_eq!(
            cache.peek(COMPACT_MIN_PREFIX, LexicalGoal::Div, context),
            Some(beta)
        );
    }

    #[test]
    fn sequential_hint_checks_context_after_insert_and_compaction() {
        let mut lexer = Lexer::new("yield /x/");
        let identifier = lexer.next_token().unwrap();
        let context = LexContext::default();
        let strict = LexContext {
            strict: true,
            ..context
        };
        lexer.seek(identifier.span.start);
        lexer.set_context(strict);
        let keyword = lexer.next_token().unwrap();
        let mut cache = LookaheadCache::default();
        for offset in 0..256 {
            cache.insert(offset, LexicalGoal::Div, context, identifier);
        }
        for offset in 128..256 {
            assert_eq!(
                cache.peek(offset, LexicalGoal::Div, context),
                Some(identifier)
            );
        }
        cache.insert(200, LexicalGoal::Div, strict, keyword);
        cache.invalidate_before(129);
        for offset in (129..256).rev() {
            assert_eq!(
                cache.peek(offset, LexicalGoal::Div, context),
                Some(identifier)
            );
        }
        assert_eq!(cache.peek(200, LexicalGoal::Div, strict), Some(keyword));
        assert_eq!(cache.peek(200, LexicalGoal::RegExp, strict), None);
        cache.invalidate_from(200);
        assert_eq!(cache.peek(200, LexicalGoal::Div, strict), None);
        assert_eq!(cache.peek(199, LexicalGoal::Div, context), Some(identifier));
    }
}
