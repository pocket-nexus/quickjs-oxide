// A specification of the VM's comparison results. Each bit corresponds to
// less, equal, greater, unordered, or an immediate right operand.
verus! {
closed spec fn accepted_orders(op: Opcode) -> u8 {
    use Opcode::*;
    match op {
        JLt | JLtI | Lt => 1,
        JLte | JLteI | Lte => 3,
        JGt | JGtI | Gt => 4,
        JGte | JGteI | Gte => 6,
        JNlt | JNltI => 14,
        JNlte | JNlteI => 12,
        JNgt | JNgtI => 11,
        JNgte | JNgteI => 9,
        JEq | JEqI | JStrictEq | JStrictEqI | Eq | StrictEq => 2,
        JNeq | JNeqI | JStrictNeq | JStrictNeqI | Neq | StrictNeq => 13,
        _ => 0,
    }
}

closed spec fn expected_mask(op: Opcode) -> u8 {
    let truth = accepted_orders(op);
    match op.format() {
        crate::bytecode::Format::AShortJump => truth | 16u8,
        _ => truth,
    }
}

// Lookup in the specified prefix of the opcode catalogue. An absent byte
// has a zero mask, including the unused tail of the 256-entry table.
closed spec fn mask_for_byte(byte: int, count: nat) -> u8
    recommends count <= Opcode::listed().len(),
    decreases count,
{
    if count == 0 {
        0
    } else {
        let op = Opcode::listed()[count - 1];
        if (op as u8) == byte {
            expected_mask(op)
        } else {
            mask_for_byte(byte, (count - 1) as nat)
        }
    }
}

proof fn mask_facts()
    ensures
        LESS == 1, EQUAL == 2, GREATER == 4, UNORDERED == 8, IMMEDIATE == 16,
        LESS | EQUAL == 3, GREATER | EQUAL == 6,
        (LESS | EQUAL | GREATER | UNORDERED) & !LESS == 14,
        (LESS | EQUAL | GREATER | UNORDERED) & !(LESS | EQUAL) == 12,
        (LESS | EQUAL | GREATER | UNORDERED) & !GREATER == 11,
        (LESS | EQUAL | GREATER | UNORDERED) & !(GREATER | EQUAL) == 9,
        (LESS | EQUAL | GREATER | UNORDERED) & !EQUAL == 13,
{
    assert((LESS, EQUAL, GREATER, UNORDERED, IMMEDIATE) == (1u8, 2u8, 4u8, 8u8, 16u8)) by(compute);
    assert(LESS | EQUAL == 3 && GREATER | EQUAL == 6) by(compute);
    assert((LESS | EQUAL | GREATER | UNORDERED) & !LESS == 14) by(compute);
    assert((LESS | EQUAL | GREATER | UNORDERED) & !(LESS | EQUAL) == 12) by(compute);
    assert((LESS | EQUAL | GREATER | UNORDERED) & !GREATER == 11) by(compute);
    assert((LESS | EQUAL | GREATER | UNORDERED) & !(GREATER | EQUAL) == 9) by(compute);
    assert((LESS | EQUAL | GREATER | UNORDERED) & !EQUAL == 13) by(compute);
}
}

macro_rules! verify_jump_masks {
    ($(#[$doc:meta])* static $name:ident: $ty:ty = { $($body:tt)* };) => {
        verus! {
            $(#[$doc])*
            exec static $name: $ty
                ensures forall|i: int| 0 <= i < 256 ==>
                    #[trigger] $name[i] == mask_for_byte(i, Opcode::listed().len()),
            {
                $($body)*
            }
        }
    };
}
