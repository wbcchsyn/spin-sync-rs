use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::{AtomicU8, Ordering};

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
///
/// The data protected by the mutex can be accessed through this guard via its
/// Deref and DerefMut implementations.
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a Mutex<T>,
    poison_flag: bool, // true if this mutex is poisoned; otherwise false.
}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    fn new(mutex: &'a Mutex<T>, status: LockState) -> Self {
        check_lock_status(status);
        debug_assert!(is_locked(status));

        let poison_flag = is_poisoned(status);
        Self { mutex, poison_flag }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        let old_status = self.mutex.lock.load(Ordering::Acquire);

        check_lock_status(old_status);
        debug_assert!(is_locked(old_status));

        let new_status = if self.poison_flag {
            POISON_UNLOCKED
        } else if std::thread::panicking() {
            POISON_UNLOCKED
        } else {
            UNLOCKED
        };

        self.mutex.lock.store(new_status, Ordering::Release);
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

//
// Marker Traits
//

impl<T: ?Sized> UnwindSafe for Mutex<T> {}
impl<T: ?Sized> RefUnwindSafe for Mutex<T> {}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

impl<T: ?Sized> !Send for MutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

//
// Constants to represent lock state
//
type LockState = u8;
const UNLOCKED: LockState = 0;
const LOCKED: LockState = 1;
const POISON_UNLOCKED: LockState = 2;
const POISON_LOCKED: LockState = 3;
const MAX_LOCK_STATE: LockState = 3;

/// Make sure the lock stauts is valid.
fn check_lock_status(s: LockState) {
    debug_assert!(s <= MAX_LOCK_STATE);
}

/// Check the status is locked or not.
fn is_locked(s: LockState) -> bool {
    check_lock_status(s);
    (s % 2) == 1
}

/// Check the status is poisoned or not.
fn is_poisoned(s: LockState) -> bool {
    check_lock_status(s);
    (s / 2) == 1
}
