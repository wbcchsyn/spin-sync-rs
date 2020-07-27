use std::sync::atomic::AtomicUsize;

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
