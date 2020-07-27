use std::sync::atomic::AtomicUsize;

pub struct Barrier {
    num_threads: usize, // immutable
    count: AtomicUsize,
    generation_id: AtomicUsize, // MSB plays lock flag role.
}

pub struct BarrierWaitResult(bool);
