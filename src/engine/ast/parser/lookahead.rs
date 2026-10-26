//! Memoized clone-lexer lookahead for the non-committing probes.
//!
//! A probe clones the committed lexer and scans forward. Scanning is a pure
//! function of `(source, offset, goal, context)`, so the same scan requested
//! by several probes, and later by the committed parse, is lexed once. The
//! cache is bounded and drops the region the parser has committed past.

use super::Parser;
use super::functions::DeclinedArrow;
use super::patterns::{BindingPatternScan, ParenthesizedParameterScan};
use super::support::ForIterationKind;
use crate::engine::ast::lexer::{LexContext, LexError, Lexer, LexicalGoal, Punctuator, Token};
use oxide_core::FxHashMap;
use std::cell::Cell;

const MAX_ENTRIES: usize = 8 * 1024;
const COMPACT_MIN_PREFIX: usize = 64;
const MAX_PARENTHESIS_SUMMARIES: usize = 1024;

#[derive(Default)]
pub(super) struct LookaheadCache<'source> {
    entries: Vec<Entry<'source>>,
    /// `entries[..base]` are invalidated and await compaction.
    base: usize,
    cursor: Cell<usize>,
    /// Whether each scanned `(` begins arrow parameters.
    parentheses: FxHashMap<usize, (LexContext, bool)>,
    parentheses_pruned_at: Option<usize>,
    pub(super) declined_arrow: Option<DeclinedArrow<'source>>,
    pub(super) binding_scan: Option<BindingScanMemo<'source>>,
    pub(super) parameter_scan: Option<(usize, LexContext, Option<ParenthesizedParameterScan>)>,
    pub(super) iteration_hint: Option<(usize, LexContext, Option<ForIterationKind>)>,
}

#[derive(Clone, Copy)]
pub(super) struct BindingScanMemo<'source> {
    pub(super) start: usize,
    pub(super) context: LexContext,
    pub(super) opening: Punctuator,
    pub(super) scan: Option<BindingPatternScan<'source>>,
    pub(super) assignment_seen: bool,
}

struct Entry<'source> {
    start: usize,
    goal: LexicalGoal,
    context: LexContext,
    token: Token<'source>,
}

impl<'source> LookaheadCache<'source> {
    fn active(&self) -> &[Entry<'source>] {
        &self.entries[self.base..]
    }

    fn peek(
        &self,
        start: usize,
        goal: LexicalGoal,
        context: LexContext,
    ) -> Option<&Token<'source>> {
        let active = self.active();
        if start < active.first()?.start || start > active.last()?.start {
            return None;
        }
        let key = (start, goal, context);
        for index in [
            self.cursor.get().saturating_add(1),
            self.cursor.get(),
            self.base,
        ] {
            if index < self.base {
                continue;
            }
            if let Some(entry) = self.entries.get(index)
                && (entry.start, entry.goal, entry.context) == key
            {
                self.cursor.set(index);
                return Some(&entry.token);
            }
        }
        let index = self
            .active()
            .binary_search_by(|entry| (entry.start, entry.goal, entry.context).cmp(&key))
            .ok()?;
        self.cursor.set(self.base + index);
        Some(&self.active()[index].token)
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
        let entry = Entry {
            start,
            goal,
            context,
            token,
        };
        if self
            .active()
            .last()
            .is_none_or(|entry| (entry.start, entry.goal, entry.context) < key)
        {
            self.entries.push(entry);
            self.cursor.set(self.entries.len() - 1);
        } else {
            let index = self.base
                + self
                    .active()
                    .partition_point(|entry| (entry.start, entry.goal, entry.context) < key);
            self.entries.insert(index, entry);
            self.cursor.set(index);
        }
    }

    fn invalidate_from(&mut self, start: usize) {
        let kept = self.active().partition_point(|entry| entry.start < start);
        self.entries.truncate(self.base + kept);
    }

    fn invalidate_before(&mut self, start: usize) {
        if self
            .active()
            .first()
            .is_none_or(|entry| entry.start >= start)
        {
            return;
        }
        self.base += if self
            .active()
            .get(1)
            .is_none_or(|entry| entry.start >= start)
        {
            1
        } else {
            self.active().partition_point(|entry| entry.start < start)
        };
        if self.base >= COMPACT_MIN_PREFIX && self.base >= self.entries.len() - self.base {
            self.cursor.update(|value| value.saturating_sub(self.base));
            self.entries.drain(..self.base);
            self.base = 0;
        }
    }
}

impl<'source> Parser<'source> {
    pub(super) fn cached_parenthesized_arrow(&self, start: usize) -> Option<bool> {
        let context = self.lexer.context();
        self.lookahead
            .borrow()
            .parentheses
            .get(&start)
            .filter(|(cached, _)| *cached == context)
            .map(|(_, arrow)| *arrow)
    }

    pub(super) fn cache_parenthesized_arrow(&self, start: usize, arrow: bool) {
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

    /// One memoized probe step from the probe lexer's position.
    pub(super) fn probe_token(
        &self,
        lexer: &mut Lexer<'source>,
        goal: LexicalGoal,
    ) -> Result<Token<'source>, LexError> {
        let context = lexer.context();
        let start = lexer.current_position().byte_offset;
        if let Some(token) = self.lookahead.borrow().peek(start, goal, context).copied() {
            lexer.seek(token.span.end);
            return Ok(token);
        }
        let token = lexer.next_token_with_goal(goal)?;
        self.lookahead
            .borrow_mut()
            .insert(start, goal, context, token);
        Ok(token)
    }

    pub(super) fn lookahead_invalidate_from(&mut self, start: usize) {
        self.lookahead.get_mut().invalidate_from(start);
    }

    /// Copy a memoized token into the committed slot.
    pub(super) fn take_lookahead(
        &mut self,
        start: usize,
        goal: LexicalGoal,
        context: LexContext,
    ) -> bool {
        let cache = self.lookahead.get_mut();
        cache.invalidate_before(start);
        let Some(entry) = cache.active().first() else {
            return false;
        };
        if entry.start != start {
            return false;
        }
        let token = if entry.goal == goal && entry.context == context {
            &entry.token
        } else if let Some(token) = cache.peek(start, goal, context) {
            token
        } else {
            return false;
        };
        self.lexer.seek(token.span.end);
        self.token = *token;
        true
    }
}
