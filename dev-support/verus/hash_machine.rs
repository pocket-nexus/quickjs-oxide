#[kani::proof]
fn initial_hash() {
    assert_eq!(FxHasher::default().finish(), 0);
}
