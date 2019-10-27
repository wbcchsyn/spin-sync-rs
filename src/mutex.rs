use std::cell::UnsafeCell;
use std::sync::atomic::AtomicU8;

/// A mutual exclusion primitive useful for protecting shared data
///
/// The interface resembles that of std::sync::Mutex.
///
/// The behavior is similar to std::sync::Mutex except for using spinlock.
/// i.e. This mutex will block threads waiting for the lock to become available.
/// Each mutex has a type parameter which represents the data that it is
/// protecting.
pub struct Mutex<T: ?Sized> {
    lock: AtomicU8,
    data: UnsafeCell<T>,
}

/// An RAII implementation of a "scoped lock" of a mutex.
///
/// When this structure is dropped (falls out of scope), the lock will be
/// unlocked.
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a Mutex<T>,
}
