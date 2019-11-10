use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::mutex::{LockResult, PoisonError, TryLockError, TryLockResult};

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
    // Each bit represents as follows.
    // - The most significant bit: poison flag
    // - The 2nd most significant bit: exclusive write lock flag
    // - The others: -> shared read lock count
    // Use helper functions for lock state.
    lock: AtomicU64,

    data: UnsafeCell<T>,
}

impl<T> RwLock<T> {
    #[must_use]
    pub fn new(t: T) -> Self {
        let lock = AtomicU64::new(INIT);
        let data = UnsafeCell::new(t);

        Self { lock, data }
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Locks this rwlock with shared read access, blocking the current thread
    /// until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which
    /// hold the lock. There may be other readers currently inside the lock when
    /// this method returns. This method does not provide any guarantees with
    /// respect to the ordering of whether contentious readers or writers will
    /// acquire the lock first.
    ///
    /// Returns an RAII guard which will release this thread's shared access
    /// once it is dropped.
    ///
    /// # Errors
    ///
    /// This function will return an error if the RwLock is poisoned. An RwLock
    /// is poisoned whenever a writer panics while holding an exclusive lock.
    /// The failure will occur immediately after the lock has been acquired.
    ///
    /// # Panics
    ///
    /// Cause panic if the maximum count of shared locks are being holded. (maximum
    /// number is 0x3fffffffffffffff.)
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::thread;
    /// use spin_sync::RwLock;
    ///
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let guard1 = lock.read().unwrap();
    /// assert_eq!(1, *guard1);
    ///
    /// let guard2 = lock.read().unwrap();
    /// assert_eq!(1, *guard2);
    /// ```
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        // Assume not poisoned, no user is holding the lock at first.
        let mut expected = INIT;

        loop {
            // Try to acquire the lock.
            let desired = acquire_shared_lock(expected);
            let current = self
                .lock
                .compare_and_swap(expected, desired, Ordering::Acquire);

            // Succeeded.
            if current == expected {
                let guard = RwLockReadGuard::new(self);

                if is_poisoned(current) {
                    return Err(PoisonError::new(guard));
                } else {
                    return Ok(guard);
                }
            }

            if is_locked_exclusively(current) {
                // Another user is holding the exclusive write lock.
                // Wait for a while and try again.
                expected = release_exclusive_lock(current);
                std::thread::yield_now();
            } else {
                // - Assumption was wrong.
                // - Another user is acquiring the shared read lock at the same time.
                // - Another user is poisoning this lock at the same time.
                // Try again soon.
                expected = current;
            }
        }
    }

    /// Attempts to acquire this rwlock with shared read access.
    ///
    /// If the access could not be granted at this time, then `Err` is returned.
    /// Otherwise, an RAII guard is returned which will release the shared access
    /// when it is dropped.
    ///
    /// This function does not block.
    ///
    /// This function does not provide any guarantees with respect to the ordering
    /// of whether contentious readers or writers will acquire the lock first.
    ///
    /// # Panics
    ///
    /// Cause panic if the maximum count of shared locks are being holded. (maximum
    /// number is 0x3fffffffffffffff.)
    ///
    /// # Errors
    ///
    /// This function will return an error if the RwLock is poisoned. An RwLock
    /// is poisoned whenever a writer panics while holding an exclusive lock. An
    /// error will only be returned if the lock would have otherwise been
    /// acquired.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let guard0 = lock.try_read().unwrap();
    /// assert_eq!(1, *guard0);
    ///
    /// let guard1 = lock.try_read().unwrap();
    /// assert_eq!(1, *guard1);
    /// ```
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<T>> {
        match self.try_lock(acquire_shared_lock, is_locked_exclusively) {
            s if is_locked_exclusively(s) => Err(TryLockError::WouldBlock),
            s if is_poisoned(s) => Err(TryLockError::Poisoned(PoisonError::new(
                RwLockReadGuard::new(self),
            ))),
            _ => Ok(RwLockReadGuard::new(self)),
        }
    }

    /// Attempts to lock this rwlock with exclusive write access.
    ///
    /// If the lock could not be acquired at this time, then `Err` is returned.
    /// Otherwise, an RAII guard is returned which will release the lock when
    /// it is dropped.
    ///
    /// This function does not block.
    ///
    /// This function does not provide any guarantees with respect to the ordering
    /// of whether contentious readers or writers will acquire the lock first.
    ///
    /// # Errors
    ///
    /// This function will return an error if the RwLock is poisoned. An RwLock
    /// is poisoned whenever a writer panics while holding an exclusive lock. An
    /// error will only be returned if the lock would have otherwise been
    /// acquired.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let mut guard = lock.try_write().unwrap();
    /// assert_eq!(1, *guard);
    ///
    /// *guard += 1;
    /// assert_eq!(2, *guard);
    ///
    /// assert!(lock.try_write().is_err());
    /// assert!(lock.try_read().is_err());
    /// ```
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<T>> {
        match self.try_lock(acquire_exclusive_lock, is_locked) {
            s if is_locked(s) => Err(TryLockError::WouldBlock),
            s if is_poisoned(s) => Err(TryLockError::Poisoned(PoisonError::new(
                RwLockWriteGuard::new(self),
            ))),
            _ => Ok(RwLockWriteGuard::new(self)),
        }
    }

    /// Locks this rwlock with exclusive write access, blocking the current
    /// thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers
    /// currently have access to the lock.
    ///
    /// Returns an RAII guard which will drop the write access of this rwlock
    /// when dropped.
    ///
    /// # Errors
    ///
    /// This function will return an error if the RwLock is poisoned. An RwLock
    /// is poisoned whenever a writer panics while holding an exclusive lock.
    /// An error will be returned when the lock is acquired.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let mut guard = lock.write().unwrap();
    /// assert_eq!(1, *guard);
    ///
    /// *guard += 1;
    /// assert_eq!(2, *guard);
    ///
    /// assert!(lock.try_read().is_err());
    /// assert!(lock.try_write().is_err());
    /// ```
    pub fn write(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
        loop {
            match self.try_lock(acquire_exclusive_lock, is_locked) {
                s if is_locked(s) => std::thread::yield_now(),
                s if is_poisoned(s) => return Err(PoisonError::new(RwLockWriteGuard::new(self))),
                _ => return Ok(RwLockWriteGuard::new(self)),
            }
        }
    }

    /// Try to acquire lock and return the lock status before updated.
    fn try_lock<AcqFn, LockCheckFn>(&self, acq_fn: AcqFn, lock_check_fn: LockCheckFn) -> LockStatus
    where
        AcqFn: Fn(LockStatus) -> LockStatus,
        LockCheckFn: Fn(LockStatus) -> bool,
    {
        // Assume not poisoned, no user is holding the lock at first.
        let mut expected = INIT;

        loop {
            // Try to acquire the lock.
            let desired = acq_fn(expected);
            let current = self
                .lock
                .compare_and_swap(expected, desired, Ordering::Acquire);

            // Succeeded.
            if current == expected {
                return current;
            }

            // Locked.
            if lock_check_fn(current) {
                return current;
            }

            // - The first assumption was wrong.
            // - Another user changes the lock status at the same time.
            // Try again soon.
            expected = current;
        }
    }
}

/// RAII structure used to release the shared read access of a lock when
/// dropped.
///
/// The data protected by the RwLock can be accessed through this guard via its
/// Deref implementation.
pub struct RwLockReadGuard<'a, T: ?Sized + 'a> {
    rwlock: &'a RwLock<T>,
}

impl<'a, T: ?Sized> RwLockReadGuard<'a, T> {
    #[must_use]
    fn new(rwlock: &'a RwLock<T>) -> Self {
        Self { rwlock }
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    /// Make sure to release the shared read lock.
    /// This function will never poison the rwlock.
    fn drop(&mut self) {
        // Assume not poisoned and no other user is holding the lock at first.
        let mut expected = acquire_shared_lock(INIT);

        loop {
            let desired = release_shared_lock(expected);
            let current = self
                .rwlock
                .lock
                .compare_and_swap(expected, desired, Ordering::Release);

            // Succeeded to release the lock.
            if current == expected {
                return;
            }

            // - Assumption was wrong.
            // - Another user release the lock at the same time.
            // Try again.
            expected = current;
        }
    }
}

/// RAII structure used to release the exclusive write access of a lock when
/// dropped.
///
/// The data protected by the RwLock can be accessed through this guard via its
/// Deref and DerefMut implementations.
pub struct RwLockWriteGuard<'a, T: ?Sized + 'a> {
    rwlock: &'a RwLock<T>,
}

impl<'a, T: ?Sized> RwLockWriteGuard<'a, T> {
    #[must_use]
    fn new(rwlock: &'a RwLock<T>) -> Self {
        Self { rwlock }
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.rwlock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    /// Make sure to release the exclusive write lock.
    ///
    /// If this user panicked, poison the lock.
    fn drop(&mut self) {
        let is_poisoned = std::thread::panicking();

        // Assume the rwlock was not poisoned at first.
        let mut expected = acquire_exclusive_lock(INIT);

        loop {
            let mut desired = release_exclusive_lock(expected);
            if is_poisoned {
                desired = set_poison_flag(desired);
            }

            let current = self
                .rwlock
                .lock
                .compare_and_swap(expected, desired, Ordering::Release);

            // Succeeded to release the lock.
            if current == expected {
                return;
            }

            // Assumption was wrong (The rwlock was already poisoned.)
            expected = current;
        }
    }
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

#[must_use]
fn release_shared_lock(s: LockStatus) -> LockStatus {
    debug_assert!(0 < count_shared_locks(s));
    s - 1
}

#[cfg(test)]
mod rwlock_tests {
    use super::*;

    #[test]
    fn try_many_times() {
        let lock = RwLock::new(0);

        // Try to write at first.
        {
            let mut guard0 = lock.try_write().unwrap();
            assert_eq!(0, *guard0);

            *guard0 += 1;
            assert_eq!(1, *guard0);

            let result1 = lock.try_read();
            assert!(result1.is_err());

            let result2 = lock.try_write();
            assert!(result2.is_err());

            let result3 = lock.try_read();
            assert!(result3.is_err());

            let result4 = lock.try_write();
            assert!(result4.is_err());
        }

        // Try to read at first.
        {
            let guard0 = lock.try_read().unwrap();
            assert_eq!(1, *guard0);

            let result1 = lock.try_write();
            assert!(result1.is_err());

            let guard2 = lock.try_read().unwrap();
            assert_eq!(1, *guard2);

            let result3 = lock.try_write();
            assert!(result3.is_err());

            let guard4 = lock.try_read().unwrap();
            assert_eq!(1, *guard4);

            let result5 = lock.try_write();
            assert!(result5.is_err());
        }
    }
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
