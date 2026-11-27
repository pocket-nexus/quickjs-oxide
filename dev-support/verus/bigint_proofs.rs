verus! {

pub open spec fn place(n: nat) -> int
    decreases n,
{
    if n == 0 { 1 } else { 0x10000000000000000int * place((n - 1) as nat) }
}

// Missing high limbs have coefficient zero. digits(s, s.len()) is the
// mathematical magnitude, without a hard-coded bound on the number of limbs.
pub open spec fn digits(words: Seq<u64>, n: nat) -> int
    decreases n,
{
    if n == 0 { 0 } else {
        let i = (n - 1) as nat;
        digits(words, i) + place(i) * (if i < words.len() { words[i as int] as int } else { 0 })
    }
}

proof fn equal_prefix(a: Seq<u64>, b: Seq<u64>, n: nat)
    requires n <= a.len(), n <= b.len(),
        forall|i: int| 0 <= i < n ==> a[i] == b[i],
    ensures digits(a, n) == digits(b, n),
    decreases n,
{
    if n > 0 { equal_prefix(a, b, (n - 1) as nat); }
}

pub closed spec fn count(b: Big) -> nat { b.limbs@.len() }

pub closed spec fn canonical(b: Big) -> bool {
    if b.limbs@.len() == 0 { !b.negative } else { b.limbs@.last() != 0 }
}

pub closed spec fn signed_value(b: Big) -> int {
    let m = digits(b.limbs@, b.limbs@.len());
    if b.negative { -m } else { m }
}

}
