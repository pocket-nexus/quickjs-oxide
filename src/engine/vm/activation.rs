/// Catch and iterator unwind metadata owned by an explicit execution frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmUnwindRegion {
    Catch {
        target: usize,
        /// Runtime operand depth before the private catch marker was
        /// installed.
        stack_depth: usize,
    },
    Iterator {
        /// Runtime operand index of `iterator`; `next` immediately follows.
        record_base: usize,
        /// `ForOfNext` disables the record before propagating its throw or
        /// publishing `done = true`, so later unwinding must not call return.
        enabled: bool,
        /// Async for-of temporarily disables the region across the cached
        /// `next` call and its Await. Keeping the record family explicit
        /// prevents sync and async continuation opcodes from crossing.
        asynchronous: bool,
    },
}
