use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Barrier {
    num_threads: usize, // immutable
    count: AtomicUsize,
    generation_id: AtomicUsize, // MSB plays lock flag role.
}

impl Barrier {
    pub const fn new(n: usize) -> Self {
        Self {
            num_threads: n,
            count: AtomicUsize::new(0),
            generation_id: AtomicUsize::new(0),
        }
    }
}

pub struct BarrierWaitResult(bool);

struct BarrierLockGuard<'a> {
    generation_id: &'a AtomicUsize,
}

impl BarrierLockGuard<'_> {
    const MSB: usize = usize::MAX / 2 + 1;
}

impl Drop for BarrierLockGuard<'_> {
    // Make sure to unlock
    fn drop(&mut self) {
        let current = self.generation_id.load(Ordering::Relaxed);
        debug_assert_eq!(Self::MSB, current & Self::MSB);

        let desired = current - Self::MSB;
        self.generation_id.store(desired, Ordering::Release);
    }
}
