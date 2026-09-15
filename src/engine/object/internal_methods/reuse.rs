//! Empty continuation allocations only: no Runtime, heap edge or depth guard is cached.
use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    thread::LocalKey,
};

// Boxes are the allocations being reused, not indirection around live owners.
#[allow(clippy::vec_box)]
pub(super) type EmptyPool<T> = LocalKey<RefCell<Vec<Box<Option<T>>>>>;

pub(super) trait Reusable: Sized + 'static {
    #[cfg(feature = "profiling")]
    const EVENT: &'static str;
    fn pool() -> &'static EmptyPool<Self>;
}

pub(super) struct PooledBox<T: Reusable>(Option<Box<Option<T>>>);
impl<T: Reusable> PooledBox<T> {
    pub(super) fn new(value: T) -> Self {
        let cached = T::pool().with(|pool| pool.borrow_mut().pop());
        let mut slot = cached.unwrap_or_else(|| {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(T::EVENT);
            Box::new(None)
        });
        *slot = Some(value);
        Self(Some(slot))
    }
    pub(super) fn into_inner(mut self) -> T {
        self.0
            .as_mut()
            .expect("continuation slot")
            .take()
            .expect("live continuation")
    }
}
impl<T: Reusable> Deref for PooledBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.as_ref().unwrap().as_ref().as_ref().unwrap()
    }
}
impl<T: Reusable> DerefMut for PooledBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0.as_mut().unwrap().as_mut().as_mut().unwrap()
    }
}
impl<T: Reusable> Drop for PooledBox<T> {
    fn drop(&mut self) {
        let Some(mut slot) = self.0.take() else {
            return;
        };
        // Drop live roots before borrowing the pool; destruction can reenter.
        drop(slot.take());
        let _ = T::pool().try_with(|pool| {
            let mut pool = pool.borrow_mut();
            if pool.len() < 16 && pool.try_reserve(1).is_ok() {
                pool.push(slot);
            }
        });
    }
}
