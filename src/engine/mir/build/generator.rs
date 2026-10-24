//! Generators and async generators: `yield`, `yield*` (QuickJS's
//! delegation loop over the inner iterator), and an async generator's
//! awaits.

use super::{Builder, Result};
use crate::engine::mir::ir::{BlockId, Imm, SuspendKind, Terminator, ThrowKind, Value, VarKind};
use crate::engine::mir::op::Op;

impl Builder<'_, '_> {
    /// Whether this function is an async generator.
    pub(super) fn async_generator(&self) -> bool {
        self.hf.async_ && self.hf.generator
    }

    /// `yield value`: an async generator awaits it first; a `.return(x)`
    /// resumption returns through the cleanups.
    pub(super) fn yield_value(&mut self, value: Value, at: u32) -> Result<Value> {
        let value = if self.async_generator() {
            self.await_value(value, Some(at))
        } else {
            value
        };
        let resume = self.block();
        let sent = self.f.new_value(Default::default());
        let kind = self.f.new_value(Default::default());
        self.end_at(
            Terminator::Suspend {
                kind: SuspendKind::Yield,
                value: Some(value),
                sent: Some(sent),
                resume_kind: Some(kind),
                resume,
            },
            at,
        );
        self.start(resume);
        let returning = self.block();
        let next = self.block();
        self.end(Terminator::Branch {
            cond: kind,
            then: returning,
            else_: next,
        });
        self.start(returning);
        self.return_completion(Some(sent), at, true)?;
        self.start(next);
        Ok(sent)
    }

    /// `yield* iterable`, QuickJS's delegation loop.
    pub(super) fn yield_star(&mut self, iterable: Value, at: u32) -> Result<Value> {
        let asynchronous = self.async_generator();
        let done_key = self.static_key("done");
        let value_key = self.static_key("value");
        let record = self.f.new_value(Default::default());
        self.record(record);
        if asynchronous {
            self.define_with(Op::IterStartAsync, &[iterable], Imm::None, &[record]);
        } else {
            self.define_with(Op::IterStart, &[iterable], Imm::Int(1), &[record]);
        }
        // What the delegate's next method receives, and its last result.
        let received = self.new_var(VarKind::Local, None);
        let result = self.new_var(VarKind::Local, None);
        let undefined = self.undefined();
        self.set_var(received, undefined);
        let step = self.block();
        let suspend = self.block();
        let end = self.block();
        self.loop_depth += 1;
        self.enter(step);
        let sent = self.get_var(received);
        let mut stepped = self.op(Op::IterCallNext, &[record, sent], Imm::None);
        if asynchronous {
            stepped = self.await_value(stepped, Some(at));
        }
        self.set_var(result, stepped);
        self.effect(Op::IterCheckObject, &[stepped], Imm::None);
        let done = self.op(Op::GetField, &[stepped], Imm::Key(done_key));
        self.end(Terminator::Branch {
            cond: done,
            then: end,
            else_: suspend,
        });
        // The suspension: the delegate's result, or in an async generator
        // its value.
        self.start(suspend);
        let current = self.get_var(result);
        let yielded = if asynchronous {
            self.op(Op::GetField, &[current], Imm::Key(value_key))
        } else {
            current
        };
        let resume = self.block();
        let resumed = self.f.new_value(Default::default());
        let kind = self.f.new_value(Default::default());
        self.end_at(
            Terminator::Suspend {
                kind: SuspendKind::YieldStar,
                value: Some(yielded),
                sent: Some(resumed),
                resume_kind: Some(kind),
                resume,
            },
            at,
        );
        self.start(resume);
        self.set_var(received, resumed);
        let abrupt = self.block();
        self.end(Terminator::Branch {
            cond: kind,
            then: abrupt,
            else_: step,
        });
        self.loop_depth -= 1;
        // `.return(x)` (kind 1) and `.throw(x)` (kind 2).
        self.start(abrupt);
        let two = self.int(2);
        let is_throw = self.op(Op::StrictEq, &[kind, two], Imm::None);
        let throwing = self.block();
        let returning = self.block();
        self.end(Terminator::Branch {
            cond: is_throw,
            then: throwing,
            else_: returning,
        });
        self.start(returning);
        let mut value = self.get_var(received);
        if asynchronous {
            value = self.await_value(value, Some(at));
        }
        let (called, missing) = self.op2(Op::IterCall, &[record, value], Imm::Int(0));
        let delegate_returned = self.block();
        let finish_return = self.block();
        let return_value = self.new_var(VarKind::Local, None);
        self.set_var(return_value, called);
        self.end(Terminator::Branch {
            cond: missing,
            then: finish_return,
            else_: delegate_returned,
        });
        self.start(delegate_returned);
        let mut outcome = self.get_var(return_value);
        if asynchronous {
            outcome = self.await_value(outcome, Some(at));
        }
        self.set_var(result, outcome);
        self.effect(Op::IterCheckObject, &[outcome], Imm::None);
        let returned_done = self.op(Op::GetField, &[outcome], Imm::Key(done_key));
        let take_value = self.block();
        self.end(Terminator::Branch {
            cond: returned_done,
            then: take_value,
            else_: suspend,
        });
        self.start(take_value);
        let done_value = self.op(Op::GetField, &[outcome], Imm::Key(value_key));
        self.set_var(return_value, done_value);
        self.goto(finish_return);
        self.start(finish_return);
        let returned = self.get_var(return_value);
        self.return_completion(Some(returned), at, true)?;
        // The throw path.
        self.start(throwing);
        let thrown = self.get_var(received);
        let (called, missing) = self.op2(Op::IterCall, &[record, thrown], Imm::Int(1));
        let delegate_threw = self.block();
        let no_throw = self.block();
        self.end(Terminator::Branch {
            cond: missing,
            then: no_throw,
            else_: delegate_threw,
        });
        self.start(delegate_threw);
        let mut outcome = called;
        if asynchronous {
            outcome = self.await_value(outcome, Some(at));
        }
        self.set_var(result, outcome);
        self.effect(Op::IterCheckObject, &[outcome], Imm::None);
        let threw_done = self.op(Op::GetField, &[outcome], Imm::Key(done_key));
        self.end(Terminator::Branch {
            cond: threw_done,
            then: end,
            else_: suspend,
        });
        // The delegate has no throw method: close it, then throw.
        self.start(no_throw);
        let undefined = self.undefined();
        let (closed, close_missing) = self.op2(Op::IterCall, &[record, undefined], Imm::Int(2));
        let throw_missing = self.block();
        if asynchronous {
            let awaiting = self.block();
            self.end(Terminator::Branch {
                cond: close_missing,
                then: throw_missing,
                else_: awaiting,
            });
            self.start(awaiting);
            self.await_value(closed, Some(at));
            self.goto(throw_missing);
        } else {
            let _ = (closed, close_missing);
            self.goto(throw_missing);
        }
        self.start(throw_missing);
        let message = self.static_key("iterator does not have a throw method");
        self.end(Terminator::ThrowError {
            kind: ThrowKind::Type,
            key: message,
        });
        // Done: the delegate's final value.
        self.start(end);
        let last = self.get_var(result);
        Ok(self.op(Op::GetField, &[last], Imm::Key(value_key)))
    }

    /// An async generator's return closing an iterator: QuickJS calls its
    /// `return` method, checks the result is an object and awaits it.
    pub(super) fn async_iterator_return(&mut self, record: Value) -> Result<()> {
        let return_key = self.static_key("return");
        // The record's iterator, as `IterDetach` keeps it.
        let iterator = self.op(Op::IterIterator, &[record], Imm::None);
        let method = self.op(Op::GetField, &[iterator], Imm::Key(return_key));
        let missing = self.op(Op::IsUndefinedOrNull, &[method], Imm::None);
        let call = self.block();
        let done: BlockId = self.block();
        self.end(Terminator::Branch {
            cond: missing,
            then: done,
            else_: call,
        });
        self.start(call);
        let result = self.op(Op::CallMethod, &[iterator, method], Imm::None);
        self.effect(Op::IterCheckObject, &[result], Imm::None);
        self.await_value(result, None);
        self.goto(done);
        self.start(done);
        Ok(())
    }
}
