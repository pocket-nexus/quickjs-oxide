//! The process-wide waiter the engine installs: one mutex over every shared
//! waitable location, one condition variable per registered waiter, and a
//! monotonic clock for the timeouts.
//!
//! The core owns the questions (a location token, an outcome, the order the
//! non-blocking byte leaves run in); this is the mechanism, and it is here
//! because it needs threads and a clock.
//!
//! The coordinator deliberately stores no runtime, heap, object, or backing
//! handles. Every operation takes the coordinator before entering a backing
//! byte leaf, establishing the fixed coordinator -> backing lock order.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use oxide_core::vm::waiter::{WaitLocation, WaitOutcome, Waiter};

/// The waiter itself: the coordinator is one process-wide static, so this
/// carries no state.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ProcessWaiter;

static ATOMIC_COORDINATOR: Mutex<AtomicCoordinator> = Mutex::new(AtomicCoordinator::new());

struct WaiterRegistration {
    id: u64,
    location: WaitLocation,
    signal: Arc<Condvar>,
}

struct AtomicCoordinator {
    next_waiter_id: u64,
    waiters: VecDeque<WaiterRegistration>,
}

impl AtomicCoordinator {
    const fn new() -> Self {
        Self {
            next_waiter_id: 1,
            waiters: VecDeque::new(),
        }
    }

    fn allocate_waiter_id(&mut self) -> u64 {
        loop {
            let id = self.next_waiter_id;
            self.next_waiter_id = self.next_waiter_id.wrapping_add(1);
            if !self.waiters.iter().any(|waiter| waiter.id == id) {
                return id;
            }
        }
    }

    fn contains(&self, id: u64) -> bool {
        self.waiters.iter().any(|waiter| waiter.id == id)
    }

    fn remove(&mut self, id: u64) -> Option<WaiterRegistration> {
        let index = self.waiters.iter().position(|waiter| waiter.id == id)?;
        self.waiters.remove(index)
    }
}

fn lock_coordinator() -> MutexGuard<'static, AtomicCoordinator> {
    // The queue remains structurally valid if a byte-leaf callback panics, so
    // a poisoned gate can be recovered instead of disabling all future atomic
    // operations in the process.
    ATOMIC_COORDINATOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Waiter for ProcessWaiter {
    fn seq_cst(&self, operation: &mut dyn FnMut()) {
        let _coordinator = lock_coordinator();
        operation()
    }

    fn wait(
        &self,
        location: WaitLocation,
        timeout: Option<u64>,
        compare: &mut dyn FnMut() -> bool,
    ) -> WaitOutcome {
        let timeout = timeout.map(Duration::from_nanos);
        let mut coordinator = lock_coordinator();
        if !compare() {
            return WaitOutcome::NotEqual;
        }

        let id = coordinator.allocate_waiter_id();
        let signal = Arc::new(Condvar::new());
        coordinator.waiters.push_back(WaiterRegistration {
            id,
            location,
            signal: Arc::clone(&signal),
        });

        match timeout {
            None => {
                while coordinator.contains(id) {
                    coordinator = signal
                        .wait(coordinator)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                WaitOutcome::Ok
            }
            Some(timeout) => {
                let started = Instant::now();
                let mut remaining = timeout;
                loop {
                    let (next, timeout_result) = signal
                        .wait_timeout(coordinator, remaining)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    coordinator = next;
                    if !coordinator.contains(id) {
                        // `notify` removed this registration first, even if the
                        // host clock expired concurrently with the wakeup.
                        return WaitOutcome::Ok;
                    }

                    let elapsed = started.elapsed();
                    if timeout_result.timed_out() || elapsed >= timeout {
                        // The timeout won while holding the same mutex used by
                        // `notify`. Removal here prevents a stale registration
                        // and makes the outcome independent of a later
                        // notification.
                        let removed = coordinator.remove(id);
                        debug_assert!(removed.is_some(), "timed-out waiter stayed discoverable");
                        return WaitOutcome::TimedOut;
                    }

                    // A non-target notification or host spurious wakeup must
                    // not extend the requested timeout or complete the wait.
                    remaining = timeout.saturating_sub(elapsed);
                }
            }
        }
    }

    fn notify(&self, location: WaitLocation, count: usize) -> usize {
        let mut coordinator = lock_coordinator();
        let mut notified = 0;
        let mut index = 0;
        while notified < count && index < coordinator.waiters.len() {
            if coordinator.waiters[index].location == location {
                let waiter = coordinator
                    .waiters
                    .remove(index)
                    .expect("indexed waiter must remain in the coordinator queue");
                waiter.signal.notify_one();
                notified += 1;
            } else {
                index += 1;
            }
        }
        notified
    }
}

#[cfg(test)]
mod tests {
    use super::{ProcessWaiter, lock_coordinator};
    use core::sync::atomic::{AtomicU64, Ordering};
    use oxide_core::vm::waiter::{WaitLocation, WaitOutcome, Waiter};
    use std::sync::mpsc;
    use std::time::Duration;

    static NEXT_TEST_BACKING_ID: AtomicU64 = AtomicU64::new(u64::MAX / 2);

    fn location(byte_offset: usize) -> WaitLocation {
        WaitLocation::new(
            NEXT_TEST_BACKING_ID.fetch_add(1, Ordering::Relaxed),
            byte_offset,
        )
    }

    #[test]
    fn failed_comparison_never_registers_a_waiter() {
        let location = location(4);
        let outcome = ProcessWaiter.wait(location, None, &mut || false);
        assert_eq!(outcome, WaitOutcome::NotEqual);
        assert_eq!(ProcessWaiter.notify(location, usize::MAX), 0);
    }

    #[test]
    fn notification_cannot_race_between_comparison_and_registration() {
        let location = location(8);
        let (compared_tx, compared_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            ProcessWaiter.wait(location, None, &mut || {
                compared_tx.send(()).unwrap();
                true
            })
        });

        compared_rx.recv().unwrap();
        assert_eq!(ProcessWaiter.notify(location, 1), 1);
        assert_eq!(worker.join().unwrap(), WaitOutcome::Ok);
        assert_eq!(ProcessWaiter.notify(location, 1), 0);
    }

    #[test]
    fn maximum_duration_wait_can_be_notified_and_removes_its_registration() {
        let location = location(8);
        let (compared_tx, compared_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            ProcessWaiter.wait(location, Some(u64::MAX), &mut || {
                compared_tx.send(()).unwrap();
                true
            })
        });

        compared_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(ProcessWaiter.notify(location, 1), 1);
        assert_eq!(worker.join().unwrap(), WaitOutcome::Ok);
        assert_eq!(ProcessWaiter.notify(location, 1), 0);
    }

    #[test]
    fn timeout_removes_the_registration() {
        let location = location(12);
        assert_eq!(
            ProcessWaiter.wait(location, Some(1_000_000), &mut || true),
            WaitOutcome::TimedOut
        );
        assert_eq!(ProcessWaiter.notify(location, 1), 0);
    }

    #[test]
    fn notify_observes_fifo_order_and_count() {
        let location = location(16);
        let (result_tx, result_rx) = mpsc::channel();
        let mut workers = Vec::new();

        for sequence in 0..3 {
            let (compared_tx, compared_rx) = mpsc::channel();
            let result_tx = result_tx.clone();
            workers.push(std::thread::spawn(move || {
                let outcome = ProcessWaiter.wait(location, None, &mut || {
                    compared_tx.send(()).unwrap();
                    true
                });
                result_tx.send((sequence, outcome)).unwrap();
            }));
            // Starting each successor only after its predecessor compared
            // fixes the registration order under the coordinator mutex.
            compared_rx.recv().unwrap();
        }
        drop(result_tx);

        assert_eq!(ProcessWaiter.notify(location, 1), 1);
        assert_eq!(result_rx.recv().unwrap(), (0, WaitOutcome::Ok));
        assert_eq!(ProcessWaiter.notify(location, 2), 2);

        let mut remaining = [result_rx.recv().unwrap(), result_rx.recv().unwrap()];
        remaining.sort_by_key(|(sequence, _)| *sequence);
        assert_eq!(remaining, [(1, WaitOutcome::Ok), (2, WaitOutcome::Ok)]);
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(ProcessWaiter.notify(location, usize::MAX), 0);
    }

    #[test]
    fn offsets_and_backings_partition_waiter_locations() {
        let first = location(20);
        let same_backing_other_offset = WaitLocation::new(first.backing_id, 24);
        let other_backing_same_offset = location(20);
        let (compared_tx, compared_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let outcome = ProcessWaiter.wait(first, None, &mut || {
                compared_tx.send(()).unwrap();
                true
            });
            result_tx.send(outcome).unwrap();
        });

        compared_rx.recv().unwrap();
        assert_eq!(ProcessWaiter.notify(same_backing_other_offset, 1), 0);
        assert_eq!(ProcessWaiter.notify(other_backing_same_offset, 1), 0);
        assert!(result_rx.recv_timeout(Duration::from_millis(5)).is_err());

        // Notify the waiter's condition variable without removing its queue
        // predicate, explicitly simulating a host spurious wakeup.
        let signal = {
            let coordinator = lock_coordinator();
            coordinator
                .waiters
                .iter()
                .find(|waiter| waiter.location == first)
                .unwrap()
                .signal
                .clone()
        };
        signal.notify_one();
        assert!(result_rx.recv_timeout(Duration::from_millis(5)).is_err());

        assert_eq!(ProcessWaiter.notify(first, 1), 1);
        assert_eq!(result_rx.recv().unwrap(), WaitOutcome::Ok);
        worker.join().unwrap();
    }

    #[test]
    fn timeout_notify_race_has_one_winner_and_leaves_no_registration() {
        for iteration in 0..32 {
            let location = location(28);
            let (compared_tx, compared_rx) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                ProcessWaiter.wait(location, Some(1_000_000), &mut || {
                    compared_tx.send(()).unwrap();
                    true
                })
            });

            compared_rx.recv().unwrap();
            let notifier = std::thread::spawn(move || {
                if iteration % 2 == 1 {
                    std::thread::sleep(Duration::from_millis(1));
                }
                ProcessWaiter.notify(location, 1)
            });
            let outcome = worker.join().unwrap();
            let notified = notifier.join().unwrap();
            assert!(matches!(
                (notified, outcome),
                (1, WaitOutcome::Ok) | (0, WaitOutcome::TimedOut)
            ));
            assert_eq!(ProcessWaiter.notify(location, usize::MAX), 0);
        }
    }

    #[test]
    fn seq_cst_gate_recovers_after_poison() {
        let _ = std::thread::spawn(|| ProcessWaiter.seq_cst(&mut || panic!("poison test"))).join();
        let mut marks = 0;
        ProcessWaiter.seq_cst(&mut || marks += 1);
        assert_eq!(marks, 1);
    }
}
