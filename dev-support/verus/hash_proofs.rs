use vstd::bytes::spec_u64_from_le_bytes;
use vstd::wrapping::u64_specs;

verus! {
impl View for FxHasher {
    type V = u64;
    closed spec fn view(&self) -> u64 { self.hash }
}

pub open spec fn mix(state: u64, word: u64) -> u64 {
    u64_specs::wrapping_mul(u64_specs::rotate_left(state, 5) ^ word, 0x517cc1b727220a95u64)
}

pub open spec fn block(bytes: Seq<u8>, i: int) -> Seq<u8> {
    bytes.subrange(8 * i, vstd::math::min(8 * i + 8, bytes.len() as int))
}

pub open spec fn padded(chunk: Seq<u8>) -> Seq<u8> {
    Seq::new(8, |i: int| if i < chunk.len() { chunk[i] } else { 0 })
}

pub open spec fn blocks(start: u64, bytes: Seq<u8>, count: nat) -> u64
    decreases count,
{
    if count == 0 { start } else {
        let i = (count - 1) as nat;
        mix(blocks(start, bytes, i), spec_u64_from_le_bytes(padded(block(bytes, i as int))))
    }
}

}

verus! {

// Each call is one event. A short final byte block is padded before the next call.
enum Write<'a> {
    Bytes(&'a [u8]),
    Byte(u8),
    U32(u32),
    U64(u64),
    Usize(usize),
}

spec fn step(s: u64, event: Write) -> u64 {
    match event {
        Write::Bytes(b) => blocks(s, b@, ((b@.len() + 7) / 8) as nat),
        Write::Byte(w) => mix(s, w as u64),
        Write::U32(w) => mix(s, w as u64),
        Write::U64(w) => mix(s, w),
        Write::Usize(w) => mix(s, w as u64),
    }
}

spec fn trace(start: u64, calls: Seq<Write>, count: nat) -> u64
    decreases count,
{
    if count == 0 { start } else {
        let i = (count - 1) as nat;
        step(trace(start, calls, i), calls[i as int])
    }
}

#[allow(dead_code)]
fn run_trace(start: u64, calls: &[Write]) -> (result: u64)
    ensures result == trace(start, calls@, calls@.len()),
{
    let mut h = FxHasher { hash: start };
    for i in 0..calls.len()
        invariant h@ == trace(start, calls@, i as nat),
    {
        match &calls[i] {
            Write::Bytes(b) => h.write(b),
            Write::Byte(w) => h.write_u8(*w),
            Write::U32(w) => h.write_u32(*w),
            Write::U64(w) => h.write_u64(*w),
            Write::Usize(w) => h.write_usize(*w),
        }
    }
    h.finish()
}

}
