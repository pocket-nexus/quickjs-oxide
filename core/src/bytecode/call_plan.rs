//! Read-only return expressions planned once, after bytecode validation.
//! A plan performs no guest call, allocation or write. A cache/type miss can
//! therefore restart the original body without replaying observable effects.

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
    Compare {
        left: u8,
        right: u8,
        left_site: u8,
        right_site: u8,
        op: Opcode,
    },
}

impl CallPlan {
    pub(super) fn of(unit: &CodeUnit) -> Self {
        use Opcode::*;
        if unit.has(flags::GENERATOR | flags::ASYNC | flags::CLASS_CONSTRUCTOR) {
            return Self::None;
        }
        let input = |r: u8| u16::from(r) <= unit.parameter_count;
        match &*unit.code {
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
    fn only_complete_read_only_expressions_are_planned() {
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
}
