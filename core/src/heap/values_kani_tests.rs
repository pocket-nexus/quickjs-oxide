// Phase 1: scalar arithmetic and encoding only; no allocation is performed.
// Domains and caller obligations: docs/formal-verification.md.
use super::*;

fn valid_class() -> u32 {
    let class: u32 = kani::any();
    // The supported classes are 0, 2..=62. Class 1 duplicates size 1.
    kani::assume(class <= 62 && class != 1);
    class
}

fn model_size(class: u32) -> u32 {
    if class % 2 == 0 {
        2_u32.pow(class / 2)
    } else {
        3 * 2_u32.pow((class - 3) / 2)
    }
}

#[kani::proof]
#[kani::unwind(7)] // u32::pow's exponent is at most 31 (five binary digits).
fn class_sizes_and_inverse() {
    let class = valid_class();
    let size = model_size(class);
    assert_eq!(class_size(class), size);
    assert_eq!(class_of(size), class);
    assert_eq!(class_len(size), size);
}

#[kani::proof]
#[kani::unwind(7)]
fn rounding_is_minimal() {
    let requested: u32 = kani::any();
    // Explicit verification domain; this does not prove all callers respect it.
    kani::assume(requested >= 1 && requested <= 1 << 31);
    let (size, class) = class_for(requested);
    assert!(class <= 62 && class != 1);
    assert_eq!(size, model_size(class));
    assert!(size >= requested);
    assert_eq!(class_len(requested), size);
    let alternative = model_size(valid_class());
    if alternative >= requested {
        assert!(size <= alternative);
    }
}

#[kani::proof]
#[kani::unwind(7)]
fn floor_is_maximal() {
    let slots: usize = kani::any();
    kani::assume(slots >= 1);
    let class = class_at_most(slots);
    assert!(class <= 62 && class != 1);
    let size = model_size(class) as usize;
    assert!(size <= slots);
    let alternative = model_size(valid_class()) as usize;
    if alternative <= slots {
        assert!(size >= alternative);
    }
}

#[kani::proof]
#[kani::unwind(7)]
fn next_class_is_the_successor() {
    let requested: u32 = kani::any();
    // Last rounded class with a successor in the supported class set.
    kani::assume(requested <= 3 << 29);
    let rounded = class_len(requested.max(1));
    let next = next_class_len(requested);
    assert!(next > rounded);
    assert!(next <= 1 << 31);
    let class = class_of(next);
    assert!(class <= 62 && class != 1);
    assert_eq!(model_size(class), next);
    let alternative = model_size(valid_class());
    if alternative > rounded {
        assert!(next <= alternative);
    }
}

#[kani::proof]
#[kani::unwind(7)]
fn range_log_round_trip() {
    let start: u32 = kani::any();
    let len = model_size(valid_class());
    // Representation round trip only: start + len need not name a live range.
    let range = Range { start, len };
    let log = range.log();
    assert_ne!(log, u8::MAX);
    assert_eq!(Range::from_log(start, log), range);
}

#[kani::proof]
fn range_exponent_round_trip() {
    let start: u32 = kani::any();
    let exponent: u8 = kani::any();
    kani::assume(exponent <= 31);
    let range = Range {
        start,
        len: 1_u32 << exponent,
    };
    assert_eq!(range.exponent(), exponent);
    assert_eq!(range.log(), exponent);
    assert_eq!(Range::from_exponent(start, exponent), range);
    assert_eq!(Range::from_log(start, exponent), range);
}

#[kani::proof]
fn empty_range_is_canonical() {
    let start: u32 = kani::any();
    let empty = Range { start, len: 0 };
    assert_eq!(class_len(0), 0);
    assert_eq!(empty.log(), u8::MAX);
    assert_eq!(empty.exponent(), u8::MAX);
    assert_eq!(Range::from_log(start, u8::MAX), Range::EMPTY);
    assert_eq!(Range::from_exponent(start, u8::MAX), Range::EMPTY);
}
