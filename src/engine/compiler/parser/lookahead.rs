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
//! entry never becomes semantically wrong. Invalidation only bounds memory and
//! drops regions the parser has already committed past.

use crate::engine::compiler::lexer::LexContext;
use crate::engine::compiler::lexer::LexError;
use crate::engine::compiler::lexer::Lexer;
use crate::engine::compiler::lexer::LexicalGoal;
use crate::engine::compiler::lexer::Token;
use crate::engine::compiler::parser::context::Parser;

#[derive(Default)]
pub(in crate::engine::compiler) struct LookaheadCache<'source> {
    entries: Vec<LookaheadEntry<'source>>,
}

struct LookaheadEntry<'source> {
    start: usize,
    goal: LexicalGoal,
    context: LexContext,
    token: Token<'source>,
}

impl<'source> LookaheadCache<'source> {
    fn peek(&self, start: usize, goal: LexicalGoal, context: LexContext) -> Option<Token<'source>> {
        let key = (start, goal, context);
        self.entries
            .binary_search_by(|entry| (entry.start, entry.goal, entry.context).cmp(&key))
            .ok()
            .map(|index| self.entries[index].token)
    }

    fn insert(
        &mut self,
        start: usize,
        goal: LexicalGoal,
        context: LexContext,
        token: Token<'source>,
    ) {
        let key = (start, goal, context);
        let index = self
            .entries
            .partition_point(|entry| (entry.start, entry.goal, entry.context) < key);
        self.entries.insert(
            index,
            LookaheadEntry {
                start,
                goal,
                context,
                token,
            },
        );
    }

    /// Drops entries that start at or after `start`, used when a goal or
    /// context change makes that whole suffix stale.
    fn invalidate_from(&mut self, start: usize) {
        let kept = self.entries.partition_point(|entry| entry.start < start);
        self.entries.truncate(kept);
    }

    /// Drops entries that start before `start`, keeping only the region the
    /// parser has not committed past.
    fn invalidate_before(&mut self, start: usize) {
        let dropped = self.entries.partition_point(|entry| entry.start < start);
        self.entries.drain(..dropped);
    }
}

impl<'source> Parser<'source> {
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

    #[cfg(test)]
    pub(in crate::engine::compiler) fn lookahead_entry_count(&self) -> usize {
        self.lookahead.borrow().entries.len()
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

    pub(in crate::engine::compiler) fn record(hit: bool) {
        let counter = if hit { &HITS } else { &MISSES };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn snapshot() -> (u64, u64) {
        (HITS.load(Ordering::Relaxed), MISSES.load(Ordering::Relaxed))
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
        assert_eq!(cache.entries.len(), 2);
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
}
