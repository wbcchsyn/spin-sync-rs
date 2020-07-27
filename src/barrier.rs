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

    pub fn wait(&self) -> BarrierWaitResult {
        let (guard, generation_id) = self.lock();

        let count = self.count.load(Ordering::Relaxed) + 1;
        self.count.store(count, Ordering::Relaxed);

        if count < self.num_threads {
            // Unlock and waiting for the leader reinitialize self.
            drop(guard);

            loop {
                let mut current_id = self.generation_id.load(Ordering::Relaxed);
                if (current_id & BarrierLockGuard::MSB) != 0 {
                    current_id = current_id - BarrierLockGuard::MSB;
                }

                if generation_id != current_id {
                    return BarrierWaitResult(false);
                } else {
                    std::thread::yield_now();
                }
            }
        } else {
            // This thread will be the leader.
            // Reinitialize self and return immediately.
            self.count.store(0, Ordering::Relaxed);

            // The other waiting threads judge whether reinitialized or not from generation_id.
            // After generation_id was updated, they stop to block and return.
            // However, the next wait() won't be started because this thread still owns the lock.
            let generation_id = (generation_id + 1) | BarrierLockGuard::MSB;
            self.generation_id.store(generation_id, Ordering::Relaxed);

            // Release the lock.
            drop(guard);

            BarrierWaitResult(true)
        }
    }

    fn lock(&self) -> (BarrierLockGuard, usize) {
        // Acquire lock
        let mut expected = 0;
        loop {
            let desired = expected + BarrierLockGuard::MSB;

            let current = self
                .generation_id
                .compare_and_swap(expected, desired, Ordering::Acquire);

            if current == expected {
                // Succeeded to lock
                break;
            } else {
                // Failed to lock.
                // Retry.
                if (current & BarrierLockGuard::MSB) != 0 {
                    // Another thread is holding the lock.
                    // Wait for a while and retry.
                    expected = current - BarrierLockGuard::MSB;
                    std::thread::yield_now();
                } else {
                    // Just the first assumption was wrong.
                    // Retry immediately.
                    expected = current;
                }
            }
        }

        (
            BarrierLockGuard {
                generation_id: &self.generation_id,
            },
            expected,
        )
    }
}

pub struct BarrierWaitResult(bool);

struct BarrierLockGuard<'a> {
    generation_id: &'a AtomicUsize,
}

impl BarrierLockGuard<'_> {
    pub const MSB: usize = usize::MAX / 2 + 1;
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
