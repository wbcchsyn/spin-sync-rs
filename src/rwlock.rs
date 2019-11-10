use std::cell::UnsafeCell;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::AtomicU64;

/// A reader-writer lock
///
/// This type of lock allows a number of readers or at most one writer at any
/// point in time. The write portion of this lock typically allows modification
/// of the underlying data (exclusive access) and the read portion of this lock
/// typically allows for read-only access (shared access).
///
/// In comparison, a Mutex does not distinguish between readers or writers
/// that acquire the lock, therefore blocking any threads waiting for the lock to
/// become available. An `RwLock` will allow any number of readers to acquire the
/// lock as long as a writer is not holding the lock.
///
/// The priority policy of the lock is dependent on the underlying operating
/// system's implementation, and this type does not guarantee that any
/// particular policy will be used.
///
/// The type parameter `T` represents the data that this lock protects. It is
/// required that `T` satisfies Send to be shared across threads and
/// Sync to allow concurrent access through readers.
///
/// # Poisoning
///
/// An `RwLock`, like `Mutex`, will become poisoned on a panic. Note, however,
/// that an `RwLock` may only be poisoned if a panic occurs while it is locked
/// exclusively (write mode). If a panic occurs in any reader, then the lock
/// will not be poisoned.
///
pub struct RwLock<T: ?Sized> {
    lock: AtomicU64,
    data: UnsafeCell<T>,
}

/// RAII structure used to release the shared read access of a lock when
/// dropped.
pub struct RwLockReadGuard<'a, T: ?Sized + 'a> {
    rwlock: &'a RwLock<T>,
}

/// RAII structure used to release the exclusive write access of a lock when
/// dropped.
pub struct RwLockWriteGuard<'a, T: ?Sized + 'a> {
    rwlock: &'a RwLock<T>,
}

//
// Marker Traits
//
impl<T: ?Sized> UnwindSafe for RwLock<T> {}
impl<T: ?Sized> RefUnwindSafe for RwLock<T> {}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}
impl<T: ?Sized> !Send for RwLockReadGuard<'_, T> {}

unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}
impl<T: ?Sized> !Send for RwLockWriteGuard<'_, T> {}

//
// Helpers for Lock State
//
type LockStatus = u64;

const INIT: LockStatus = 0;
const SHARED_LOCK_MASK: LockStatus = 0x3fffffffffffffff;
const EXCLUSIVE_LOCK_FLAG: LockStatus = 0x4000000000000000;
const POISON_FLAG: LockStatus = 0x8000000000000000;

#[must_use]
fn is_poisoned(s: LockStatus) -> bool {
    (s & POISON_FLAG) != 0
}

#[must_use]
fn set_poison_flag(s: LockStatus) -> LockStatus {
    s | POISON_FLAG
}

#[must_use]
fn is_locked(s: LockStatus) -> bool {
    s & (!POISON_FLAG) != 0
}

#[must_use]
fn is_locked_exclusively(s: LockStatus) -> bool {
    let ret = (s & EXCLUSIVE_LOCK_FLAG) != 0;

    if ret {
        debug_assert_eq!(0, s & SHARED_LOCK_MASK);
    }

    ret
}

#[must_use]
fn acquire_exclusive_lock(s: LockStatus) -> LockStatus {
    debug_assert_eq!(false, is_locked(s));
    s | EXCLUSIVE_LOCK_FLAG
}

#[must_use]
fn release_exclusive_lock(s: LockStatus) -> LockStatus {
    debug_assert_eq!(true, is_locked_exclusively(s));
    s & (!EXCLUSIVE_LOCK_FLAG)
}

#[must_use]
fn count_shared_locks(s: LockStatus) -> u64 {
    let ret = s & SHARED_LOCK_MASK;

    if 0 < ret {
        debug_assert_eq!(0, s & EXCLUSIVE_LOCK_FLAG);
    }

    ret
}

/// # Panic
///
/// Cause panic if the maximum count of shared locks are being holded. (maximum
/// number is 0x3fffffffffffffff.)
#[must_use]
fn acquire_shared_lock(s: LockStatus) -> LockStatus {
    debug_assert_eq!(false, is_locked_exclusively(s));

    if count_shared_locks(s) == SHARED_LOCK_MASK {
        panic!("rwlock maximum reader count exceeded");
    }

    s + 1
}

#[cfg(test)]
mod lock_state_tests {
    use super::*;

    #[test]
    fn flag_duplication() {
        assert_eq!(0, INIT & SHARED_LOCK_MASK);
        assert_eq!(0, INIT & EXCLUSIVE_LOCK_FLAG);
        assert_eq!(0, INIT & POISON_FLAG);
        assert_eq!(0, SHARED_LOCK_MASK & EXCLUSIVE_LOCK_FLAG);
        assert_eq!(0, SHARED_LOCK_MASK & POISON_FLAG);
        assert_eq!(0, EXCLUSIVE_LOCK_FLAG & POISON_FLAG);
    }

    #[test]
    fn flag_uses_all_bits() {
        assert_eq!(
            std::u64::MAX,
            INIT | SHARED_LOCK_MASK | EXCLUSIVE_LOCK_FLAG | POISON_FLAG
        );
    }
}
