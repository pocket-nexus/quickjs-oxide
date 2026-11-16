//! Short completions planned once, after bytecode validation. Reads perform
//! no guest call or allocation. A store commits only after every guard has
//! passed, with no later miss, so fallback never replays an observable write.

use super::{CodeUnit, Opcode, flags};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallPlan {
    None,
    Register(u8),
    Constant {
        op: Opcode,
        imm: u32,
    },
    Field {
        base: u8,
        site: u8,
    },
    Chain {
        base: u8,
        first: u8,
        second: u8,
    },
    Element {
        base: u8,
        site: u8,
        index: u8,
    },
    Store {
        base: u8,
        value: u8,
        site: u8,
    },
    StoreConstant {
        base: u8,
        site: u8,
        op: Opcode,
        imm: u32,
    },
    Update {
        base: u8,
        read: u8,
        global: u16,
        op: Opcode,
        write: u8,
    },
    BitTestOrCompare {
        base: u8,
        left: u8,
        left_global: u8,
        right: u8,
        right_global: u8,
        op: Opcode,
    },
    Delegate {
        base: u8,
        receiver: u8,
        method: u8,
        call: u8,
        argument: u8,
        argc: u8,
    },
    Transfer {
        base: u8,
        target: u8,
        source: u8,
        target_call: u8,
        source_call: u8,
        read: u8,
        write: u8,
    },
    GlobalStore {
        base: u8,
        global: u16,
        read_site: u8,
        write_site: u8,
    },
    Select {
        base: u8,
        left_site: u8,
        global: u8,
        right_site: u8,
        op: Opcode,
        taken: u8,
        fallthrough: u8,
    },
    GlobalCompare {
        left: u8,
        left_site: u8,
        global: u16,
        right_site: u8,
        op: Opcode,
    },
    Compare {
        left: u8,
        right: u8,
        left_site: u8,
        right_site: u8,
        op: Opcode,
    },
}

const _: () = assert!(size_of::<CallPlan>() == 8);

impl CallPlan {
    pub(super) fn of(unit: &CodeUnit) -> Self {
        use Opcode::*;
        if unit.has(flags::GENERATOR | flags::ASYNC | flags::CLASS_CONSTRUCTOR) {
            return Self::None;
        }
        let input = |r: u8| u16::from(r) <= unit.parameter_count;
        let constant = |op| {
            matches!(
                op,
                LoadInt | LoadConst | LoadUndefined | LoadNull | LoadTrue | LoadFalse
            )
        };
        let returns_undefined = |load: &super::Insn, ret: &super::Insn| {
            load.op == LoadUndefined && ret.op == Return && load.a == ret.a
        };
        let delegate = |receiver: &super::Insn,
                        method: &super::Insn,
                        call: &super::Insn,
                        ret: &super::Insn| {
            receiver.op == GetField
                && input(receiver.b)
                && !input(receiver.a)
                && method.op == GetField
                && method.b == receiver.a
                && !input(method.a)
                && call.op == CallMethod
                && call.b == receiver.a
                && u16::from(method.a) == u16::from(call.b) + 1
                && ret.op == Return
                && ret.a == call.a
        };
        match &*unit.code {
            [
                target,
                target_this,
                target_call,
                source,
                source_this,
                source_call,
                read,
                write,
                undef,
                ret,
            ] if target.op == GetField
                && input(target.b)
                && target_this.op == Mov
                && target_this.b == target.b
                && !input(target_this.a)
                && target_call.op == CallMethod
                && target_call.b == target_this.a
                && target_call.a == target_call.b
                && target_call.c == 0
                && u16::from(target.a) == u16::from(target_call.b) + 1
                && source.op == GetField
                && source.b == target.b
                && source_this.op == Mov
                && source_this.b == target.b
                && !input(source_this.a)
                && source_call.op == CallMethod
                && source_call.b == source_this.a
                && source_call.a == source_call.b
                && source_call.c == 0
                && source_call.b > target_call.b
                && u16::from(source.a) == u16::from(source_call.b) + 1
                && read.op == GetField
                && read.b == source_call.a
                && read.a == source_call.a
                && write.op == SetField
                && write.a == target_call.a
                && write.b == read.a
                && returns_undefined(undef, ret) =>
            {
                Self::Transfer {
                    base: target.b,
                    target: target.c,
                    source: source.c,
                    target_call: target_call.b,
                    source_call: source_call.b,
                    read: read.c,
                    write: write.c,
                }
            }
            [receiver, method, call, ret]
                if delegate(receiver, method, call, ret) && call.c == 0 =>
            {
                Self::Delegate {
                    base: receiver.b,
                    receiver: receiver.c,
                    method: method.c,
                    call: call.b,
                    argument: 0,
                    argc: 0,
                }
            }
            [receiver, method, argument, call, ret]
                if delegate(receiver, method, call, ret)
                    && call.c == 1
                    && argument.op == Mov
                    && input(argument.b)
                    && !input(argument.a)
                    && u16::from(argument.a) == u16::from(call.b) + 2 =>
            {
                Self::Delegate {
                    base: receiver.b,
                    receiver: receiver.c,
                    method: method.c,
                    call: call.b,
                    argument: argument.b,
                    argc: 1,
                }
            }
            [
                left,
                global,
                and,
                zero,
                test,
                copy,
                branch,
                right,
                other,
                compare,
                ret,
            ] if left.op == GetField
                && input(left.b)
                && !input(left.a)
                && matches!(global.op, GetGlobal | GetGlobalOrUndefined)
                && !input(global.a)
                && global.a != left.a
                && global.c == 0
                && and.op == BitAnd
                && and.a == left.a
                && and.b == left.a
                && and.c == global.a
                && zero.op == LoadInt
                && zero.int() == 0
                && !input(zero.a)
                && zero.a != and.a
                && matches!(test.op, Neq | StrictNeq)
                && test.a == left.a
                && test.b == and.a
                && test.c == zero.a
                && copy.op == Mov
                && copy.b == test.a
                && !input(copy.a)
                && branch.op == JTrue
                && branch.a == copy.a
                && branch.imm == 10
                && right.op == GetField
                && right.b == left.b
                && !input(right.a)
                && matches!(other.op, GetGlobal | GetGlobalOrUndefined)
                && !input(other.a)
                && other.a != right.a
                && other.c == 0
                && matches!(
                    compare.op,
                    Lt | Lte | Gt | Gte | Eq | Neq | StrictEq | StrictNeq
                )
                && compare.b == right.a
                && compare.c == other.a
                && ret.op == Return
                && ret.a == test.a
                && ret.a == compare.a =>
            {
                Self::BitTestOrCompare {
                    base: left.b,
                    left: left.c,
                    left_global: global.b,
                    right: right.c,
                    right_global: other.b,
                    op: compare.op,
                }
            }
            [read, global, op, write, undef, ret]
                if read.op == GetField
                    && input(read.b)
                    && !input(read.a)
                    && matches!(global.op, GetGlobal | GetGlobalOrUndefined)
                    && !input(global.a)
                    && global.a != read.a
                    && matches!(op.op, BitAnd | BitOr | BitXor | Shl | Sar | Shr)
                    && op.a == read.a
                    && op.b == read.a
                    && op.c == global.a
                    && write.op == SetField
                    && write.a == read.b
                    && write.b == op.a
                    && returns_undefined(undef, ret) =>
            {
                Self::Update {
                    base: read.b,
                    read: read.c,
                    global: u16::from(global.b) | (u16::from(global.c) << 8),
                    op: op.op,
                    write: write.c,
                }
            }
            [left, global, right, branch, yes, jump, no, ret]
                if left.op == GetField
                    && input(left.b)
                    && !input(left.a)
                    && matches!(global.op, GetGlobal | GetGlobalOrUndefined)
                    && !input(global.a)
                    && global.a != left.a
                    && global.c == 0
                    && right.op == GetField
                    && right.b == global.a
                    && !input(right.a)
                    && right.a != left.a
                    && matches!(
                        branch.op,
                        JLt | JLte
                            | JGt
                            | JGte
                            | JNlt
                            | JNlte
                            | JNgt
                            | JNgte
                            | JEq
                            | JNeq
                            | JStrictEq
                            | JStrictNeq
                    )
                    && branch.a == left.a
                    && branch.b == right.a
                    && branch.imm == 6
                    && yes.op == GetField
                    && no.op == GetField
                    && yes.b == left.b
                    && no.b == left.b
                    && jump.op == Jmp
                    && jump.imm == 7
                    && ret.op == Return
                    && ret.a == yes.a
                    && ret.a == no.a =>
            {
                Self::Select {
                    base: left.b,
                    left_site: left.c,
                    global: global.b,
                    right_site: right.c,
                    op: branch.op,
                    taken: no.c,
                    fallthrough: yes.c,
                }
            }
            [ret] if ret.op == Return && input(ret.a) => Self::Register(ret.a),
            [load, ret] if ret.op == Return && ret.a == load.a => match load.op {
                LoadInt | LoadConst | LoadUndefined | LoadNull | LoadTrue | LoadFalse => {
                    Self::Constant {
                        op: load.op,
                        imm: load.imm,
                    }
                }
                GetField if input(load.b) => Self::Field {
                    base: load.b,
                    site: load.c,
                },
                _ => Self::None,
            },
            [first, second, ret]
                if first.op == SetField
                    && input(first.a)
                    && input(first.b)
                    && returns_undefined(second, ret) =>
            {
                Self::Store {
                    base: first.a,
                    value: first.b,
                    site: first.c,
                }
            }
            [load, write, undef, ret]
                if constant(load.op)
                    && write.op == SetField
                    && input(write.a)
                    && write.b == load.a
                    && write.a != load.a
                    && returns_undefined(undef, ret) =>
            {
                Self::StoreConstant {
                    base: write.a,
                    site: write.c,
                    op: load.op,
                    imm: load.imm,
                }
            }
            [global, read, write, undef, ret]
                if matches!(global.op, GetGlobal | GetGlobalOrUndefined)
                    && read.op == GetField
                    && read.b == global.a
                    && write.op == SetField
                    && input(write.a)
                    && write.b == read.a
                    && write.a != global.a
                    && write.a != read.a
                    && returns_undefined(undef, ret) =>
            {
                Self::GlobalStore {
                    base: write.a,
                    global: u16::from(global.b) | (u16::from(global.c) << 8),
                    read_site: read.c,
                    write_site: write.c,
                }
            }
            [first, second, ret]
                if first.op == GetField
                    && second.op == GetElem
                    && input(first.b)
                    && input(second.c)
                    && first.a != second.c
                    && second.b == first.a
                    && ret.op == Return
                    && ret.a == second.a =>
            {
                Self::Element {
                    base: first.b,
                    site: first.c,
                    index: second.c,
                }
            }
            [first, second, ret]
                if first.op == GetField
                    && second.op == GetField
                    && input(first.b)
                    && second.b == first.a
                    && ret.op == Return
                    && ret.a == second.a =>
            {
                Self::Chain {
                    base: first.b,
                    first: first.c,
                    second: second.c,
                }
            }
            [left, right, compare, ret]
                if left.op == GetField
                    && right.op == GetField
                    && input(left.b)
                    && input(right.b)
                    && !input(left.a)
                    && !input(right.a)
                    && left.a != right.a
                    && compare.b == left.a
                    && compare.c == right.a
                    && matches!(
                        compare.op,
                        Lt | Lte | Gt | Gte | Eq | Neq | StrictEq | StrictNeq
                    )
                    && ret.op == Return
                    && ret.a == compare.a =>
            {
                Self::Compare {
                    left: left.b,
                    right: right.b,
                    left_site: left.c,
                    right_site: right.c,
                    op: compare.op,
                }
            }
            [left, global, right, compare, ret]
                if left.op == GetField
                    && input(left.b)
                    && !input(left.a)
                    && matches!(global.op, GetGlobal | GetGlobalOrUndefined)
                    && global.a != left.a
                    && right.op == GetField
                    && right.b == global.a
                    && right.a != left.a
                    && compare.b == left.a
                    && compare.c == right.a
                    && matches!(
                        compare.op,
                        Lt | Lte | Gt | Gte | Eq | Neq | StrictEq | StrictNeq
                    )
                    && ret.op == Return
                    && ret.a == compare.a =>
            {
                Self::GlobalCompare {
                    left: left.b,
                    left_site: left.c,
                    global: u16::from(global.b) | (u16::from(global.c) << 8),
                    right_site: right.c,
                    op: compare.op,
                }
            }
            _ => Self::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{CodeBuilder, Insn, builder::UnitLayout};

    fn plan(code: &[Insn], parameters: u16) -> CallPlan {
        let mut b = CodeBuilder::new();
        for &insn in code {
            b.raw(insn);
        }
        let unit = b.finish(UnitLayout {
            register_count: 8,
            parameter_count: parameters,
            ..UnitLayout::default()
        });
        CallPlan::of(&unit)
    }

    #[test]
    fn only_complete_expressions_or_single_stores_are_planned() {
        let get = Insn::new(Opcode::GetField, 3, 1, 0, 0);
        let right = Insn::new(Opcode::GetField, 4, 2, 1, 0);
        let compare = Insn::new(Opcode::Lt, 3, 3, 4, 0);
        let ret = Insn::new(Opcode::Return, 3, 0, 0, 0);
        assert!(matches!(
            plan(&[get, ret], 2),
            CallPlan::Field { base: 1, .. }
        ));
        assert!(matches!(
            plan(&[get, right, compare, ret], 2),
            CallPlan::Compare {
                left: 1,
                right: 2,
                ..
            }
        ));
        let mut alias = right;
        alias.b = get.a;
        assert_eq!(plan(&[get, alias, compare, ret], 2), CallPlan::None);
        let write = Insn::new(Opcode::SetField, 1, 2, 0, 0);
        assert_eq!(plan(&[write, get, ret], 2), CallPlan::None);
        let call = Insn::new(Opcode::Call, 3, 1, 0, 0);
        assert_eq!(plan(&[call, ret], 2), CallPlan::None);
    }

    #[test]
    fn bit_plans_require_original_receivers_and_exact_short_circuit_targets() {
        let get = Insn::new(Opcode::GetField, 1, 0, 0, 0);
        let global = Insn::new(Opcode::GetGlobal, 2, 0, 0, 0);
        let ret = Insn::new(Opcode::Return, 1, 0, 0, 0);
        let update = [
            get,
            global,
            Insn::new(Opcode::BitOr, 1, 1, 2, 0),
            Insn::new(Opcode::SetField, 0, 1, 0, 0),
            Insn::new(Opcode::LoadUndefined, 1, 0, 0, 0),
            ret,
        ];
        assert!(matches!(plan(&update, 0), CallPlan::Update { base: 0, .. }));
        let mut alias = update;
        alias[1].a = 0;
        assert_eq!(plan(&alias, 0), CallPlan::None);
        let test = [
            get,
            global,
            Insn::new(Opcode::BitAnd, 1, 1, 2, 0),
            Insn::new(Opcode::LoadInt, 2, 0, 0, 0),
            Insn::new(Opcode::Neq, 1, 1, 2, 0),
            Insn::new(Opcode::Mov, 2, 1, 0, 0),
            Insn::new(Opcode::JTrue, 2, 0, 0, 10),
            get,
            global,
            Insn::new(Opcode::Eq, 1, 1, 2, 0),
            ret,
        ];
        assert!(matches!(
            plan(&test, 0),
            CallPlan::BitTestOrCompare { base: 0, .. }
        ));
        let mut alias = test;
        alias[5].a = 0;
        assert_eq!(plan(&alias, 0), CallPlan::None);
        let mut wrong_target = test;
        wrong_target[6].imm = 9;
        assert_eq!(plan(&wrong_target, 0), CallPlan::None);
    }

    #[test]
    fn delegated_calls_preserve_the_original_argument_and_method_window() {
        let code = [
            Insn::new(Opcode::GetField, 2, 0, 0, 0),
            Insn::new(Opcode::GetField, 3, 2, 0, 0),
            Insn::new(Opcode::Mov, 4, 1, 0, 0),
            Insn::new(Opcode::CallMethod, 2, 2, 1, 0),
            Insn::new(Opcode::Return, 2, 0, 0, 0),
        ];
        assert!(matches!(
            plan(&code, 1),
            CallPlan::Delegate {
                argument: 1,
                argc: 1,
                ..
            }
        ));
        let mut overwritten = code;
        overwritten[0].a = 1;
        assert_eq!(plan(&overwritten, 1), CallPlan::None);
        let mut alias = code;
        alias[2].a = 3;
        assert_eq!(plan(&alias, 1), CallPlan::None);
    }

    #[test]
    fn transfers_keep_the_first_result_below_the_second_call_window() {
        let code = [
            Insn::new(Opcode::GetField, 2, 0, 0, 0),
            Insn::new(Opcode::Mov, 1, 0, 0, 0),
            Insn::new(Opcode::CallMethod, 1, 1, 0, 0),
            Insn::new(Opcode::GetField, 3, 0, 0, 0),
            Insn::new(Opcode::Mov, 2, 0, 0, 0),
            Insn::new(Opcode::CallMethod, 2, 2, 0, 0),
            Insn::new(Opcode::GetField, 2, 2, 0, 0),
            Insn::new(Opcode::SetField, 1, 2, 0, 0),
            Insn::new(Opcode::LoadUndefined, 1, 0, 0, 0),
            Insn::new(Opcode::Return, 1, 0, 0, 0),
        ];
        assert!(matches!(plan(&code, 0), CallPlan::Transfer { .. }));
        let mut alias = code;
        alias[3].a = 2;
        alias[4].a = 1;
        alias[5].a = 1;
        alias[5].b = 1;
        alias[6].a = 1;
        alias[6].b = 1;
        alias[7].b = 1;
        assert_eq!(plan(&alias, 0), CallPlan::None);
    }
}
