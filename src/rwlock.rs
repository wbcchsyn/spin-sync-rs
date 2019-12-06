use std::cell::UnsafeCell;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::misc::PhantomNotSend;
use crate::result::{LockResult, PoisonError, TryLockError, TryLockResult};

/// A reader-writer lock.
///
/// It behaves like std::sync::RwLock except for using spinlock.
///
/// This type of lock allows either a number of readers or at most one writer
/// at the same time. Readers are allowed read-only access (shared access)
/// to the underlying data while the writer is allowed read/write access
/// (exclusive access.)
///
/// In comparison, a [`Mutex`] does not distinguish between readers and writers,
/// therefore blocking any threads waiting for the lock to become available.
/// An `RwLock` will allow any number of readers to acquire the lock as long as
/// a writer is not holding the lock.
///
/// There is no priority difference with respect to the ordering of
/// whether contentious readers or writers will acquire the lock first.
///
/// # Poisoning
///
/// An `RwLock`, like [`Mutex`], will become poisoned on a panic. Note, however,
/// that an `RwLock` may only be poisoned if a panic occurs while it is locked
/// exclusively (write mode). If a panic occurs in any reader, then the lock
/// will not be poisoned.
///
/// [`Mutex`]: struct.Mutex.html
///
/// # Examples
///
/// Create a variable protected by a RwLock, increment it by 2 in worker threads
/// at the same time, and check the variable was updated rightly.
///
/// ```
/// use spin_sync::RwLock;
/// use std::sync::Arc;
/// use std::thread;
///
/// const WORKER_NUM: usize = 10;
/// let mut handles = Vec::with_capacity(WORKER_NUM);
///
/// // Decrare a variable protected by RwLock.
/// // It is wrapped in std::Arc to share this instance itself among threads.
/// let rwlock = Arc::new(RwLock::new(0));
///
/// // Create worker threads to inclement the value by 2.
/// for _ in 0..WORKER_NUM {
///     let c_rwlock = rwlock.clone();
///
///     let handle = thread::spawn(move || {
///         let mut num = c_rwlock.write().unwrap();
///         *num += 2;
///     });
///
///     handles.push(handle);
/// }
///
/// // Make sure the value is always multipile of 2 even if some worker threads
/// // are working.
/// //
/// // Enclosing the lock with `{}` to drop it before waiting for the worker
/// // threads; otherwise, deadlocks could be occurred.
/// {
///     let num = rwlock.read().unwrap();
///     assert_eq!(0, *num % 2);
/// }
///
/// // Wait for the all worker threads are finished.
/// for handle in handles {
///     handle.join().unwrap();
/// }
///
/// // Make sure the value is incremented by 2 times the worker count.
/// let num = rwlock.read().unwrap();
/// assert_eq!(2 * WORKER_NUM, *num);
/// ```
pub struct RwLock<T: ?Sized> {
    // Each bit represents as follows.
    // - The most significant bit: poison flag
    // - The 2nd most significant bit: exclusive write lock flag
    // - The others: shared read lock count
    // Use helper functions for lock state.
    lock: AtomicU64,

    data: UnsafeCell<T>,
}

impl<T> RwLock<T> {
    /// Creates a new instance in unlocked state ready for use.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::RwLock;
    ///
    /// let lock = RwLock::new(5);
    /// ```
    #[inline]
    #[must_use]
    pub fn new(t: T) -> Self {
        let lock = AtomicU64::new(INIT);
        let data = UnsafeCell::new(t);

        Self { lock, data }
    }

    /// Consumes this instance and returns the underlying data.
    ///
    /// Note that this method won't acquire any lock because we know there is
    /// no other references to `self`.
    ///
    /// # Errors
    ///
    /// If another user panicked while holding the exclusive write lock of this instance,
    /// this method call wraps the guard in an error and returns it.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::RwLock;
    ///
    /// let rwlock = RwLock::new(0);
    /// assert_eq!(0, rwlock.into_inner().unwrap());
    /// ```
    #[inline]
    pub fn into_inner(self) -> LockResult<T> {
        // We know statically that there are no outstanding references to
        // `self` so there's no need to lock the inner lock.
        let is_err = self.is_poisoned();
        let data = self.data.into_inner();

        if is_err {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }
}

impl<T: ?Sized> RwLock<T> {
    /// The maximum shared read locks of each instance.
    pub const MAX_READ_LOCK_COUNT: u64 = SHARED_LOCK_MASK;

    /// Blocks the current thread until acquiring a shared read lock, and
    /// returns an RAII guard object.
    ///
    /// The actual flow will be as follows.
    ///
    /// 1. User calls this method.
    ///    1. Blocks until this thread acquires a shared read lock
    ///       (i.e. until the exclusive write lock is held.)
    ///    1. Creates an RAII guard object.
    ///    1. Wrapps the guard in `Result` and returns it. If this instance has been
    ///       poisoned, it is wrapped in an `Err`; otherwise wrapped in an `Ok`.
    /// 1. User accesses to the underlying data to read through the guard.
    ///    (No write access is then.)
    /// 1. The guard is dropped (falls out of scope) and the lock is released.
    ///
    /// # Errors
    ///
    /// If another user panicked while holding the exclusive write lock of this instance,
    /// this method call wraps the guard in an error and returns it.
    ///
    /// # Panics
    ///
    /// This method panics if `MAX_READ_LOCK_COUNT` shared locks are.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let guard1 = lock.read().unwrap();
    /// assert_eq!(1, *guard1);
    ///
    /// let guard2 = lock.read().unwrap();
    /// assert_eq!(1, *guard2);
    /// ```
    #[inline]
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        loop {
            match self.try_lock(acquire_shared_lock, is_locked_exclusively) {
                s if is_locked_exclusively(s) => std::thread::yield_now(),
                s if is_poisoned(s) => return Err(PoisonError::new(RwLockReadGuard::new(self))),
                _ => return Ok(RwLockReadGuard::new(self)),
            }
        }
    }

    /// Attempts to acquire a shared read lock and returns an RAII guard object if succeeded.
    ///
    /// Behaves like [`read`] except for this method returns an error immediately
    /// if the exclusive write lock is being held.
    ///
    /// This function does not block.
    ///
    /// The actual flow will be as follows.
    ///
    /// 1. User calls this method.
    ///    1. Tries to acquire a shared read lock. If failed (i.e. if the exclusive write
    ///       lock is being held,) returns an error immediately and this flow is finished here.
    ///    1. Creates an RAII guard object.
    ///    1. Wrapps the guard in `Result` and returns it. If this instance has been poisoned,
    ///       it is wrapped in an `Err`; otherwise wrapped in an `Ok`.
    /// 1. User accesses to the underlying data to read through the guard.
    ///    (No write access is at then.)
    /// 1. The guard is dropped (falls out of scope) and the lock is released.
    ///
    /// [`read`]: #method.read
    ///
    /// # Panics
    ///
    /// This method panics if `MAX_READ_LOCK` shared read locks are.
    ///
    /// # Errors
    ///
    /// - If another user is holding the exclusive write lock,
    ///   [`TryLockError::WouldBlock`] is returned.
    /// - If this method call succeeded to acquire a shared read lock, and if another
    ///   user had panicked while holding the exclusive write lock,
    ///   [`TryLockError::Poisoned`] is returned.
    ///
    /// [`TryLockError::WouldBlock`]: type.TryLockError.html
    /// [`TryLockError::Poisoned`]: type.TryLockError.html
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
    #[inline]
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<T>> {
        match self.try_lock(acquire_shared_lock, is_locked_exclusively) {
            s if is_locked_exclusively(s) => Err(TryLockError::WouldBlock),
            s if is_poisoned(s) => Err(TryLockError::Poisoned(PoisonError::new(
                RwLockReadGuard::new(self),
            ))),
            _ => Ok(RwLockReadGuard::new(self)),
        }
    }

    /// Attempts to acquire the exclusive write lock and returns an RAII guard object
    /// if succeeded.
    ///
    /// Behaves like [`write`] except for this method returns an error immediately
    /// if any other lock (either read lock or write lock) is being held.
    ///
    /// This method does not block.
    ///
    /// The actual flow will be as follows.
    ///
    /// 1. User calls this method.
    ///    1. Tries to acquire the exclusive write lock. If failed (i.e. if any other lock is
    ///       being held,) returns an error immediately and this flow is finished here.
    ///    1. Creates an RAII guard object.
    ///    1. Wraps the guard in `Result` and returns it. If this instance has been poisoned,
    ///       it is wrapped in an `Err`; otherwise wrapped in an `Ok`.
    /// 1. User accesses to the underlying data to read/write through the guard.
    ///    (No other access is then.)
    /// 1. The guard is dropped (falls out of scope) and the lock is released.
    ///
    /// [`write`]: #method.write
    ///
    /// # Errors
    ///
    /// - If another user is holding any other lock (either read lock or write lock),
    ///   [`TryLockError::WouldBlock`] is returned.
    /// - If this method call succeeded to acquire the lock, and if another user had panicked
    ///   while holding the exclusive write lock, [`TryLockError::Poisoned`] is returned.
    ///
    /// [`TryLockError::WouldBlock`]: type.TryLockError.html
    /// [`TryLockError::Poisoned`]: type.TryLockError.html
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
    #[inline]
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<T>> {
        match self.try_lock(acquire_exclusive_lock, is_locked) {
            s if is_locked(s) => Err(TryLockError::WouldBlock),
            s if is_poisoned(s) => Err(TryLockError::Poisoned(PoisonError::new(
                RwLockWriteGuard::new(self),
            ))),
            _ => Ok(RwLockWriteGuard::new(self)),
        }
    }

    /// Blocks the current thread until acquiring the exclusive write lock, and
    /// returns an RAII guard object.
    ///
    /// The actual flow will be as follows.
    ///
    /// 1. User calls this method.
    ///    1. Blocks until this thread acquires the exclusive write lock
    ///       (i.e. until any other lock is held.)
    ///    1. Creates an RAII guard object.
    ///    1. Wrapps the guard in Result and returns it. If this instance has been
    ///       poisoned, it is wrapped in an `Err`; otherwise wrapped in an `Ok`.
    /// 1. User accesses to the underlying data to read/write through the guard.
    ///    (No other access is then.)
    /// 1. The guard is dropped (falls out of scope) and the lock is released.
    ///
    /// # Errors
    ///
    /// If another user panicked while holding the exclusive write lock of this instance,
    /// this method call wraps the guard in an error and returns it.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::RwLock;
    ///
    /// let lock = RwLock::new(0);
    ///
    /// let mut guard = lock.write().unwrap();
    /// assert_eq!(0, *guard);
    ///
    /// *guard += 1;
    /// assert_eq!(1, *guard);
    ///
    /// assert_eq!(true, lock.try_read().is_err());
    /// assert_eq!(true, lock.try_write().is_err());
    /// ```
    #[inline]
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

    /// Determines whether the lock is poisoned or not.
    ///
    /// # Warning
    ///
    /// This function won't acquire any lock. If another thread is active,
    /// the rwlock can become poisoned at any time. You should not trust a `false`
    /// value for program correctness without additional synchronization.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::RwLock;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let lock = Arc::new(RwLock::new(0));
    /// assert_eq!(false, lock.is_poisoned());
    ///
    /// {
    ///     let lock = lock.clone();
    ///
    ///     let _ = thread::spawn(move || {
    ///         // This panic while holding the lock (`_guard` is in scope) will poison
    ///         // the instance.
    ///         let _guard = lock.write().unwrap();
    ///         panic!("Poison here");
    ///     }).join();
    /// }
    ///
    /// assert_eq!(true, lock.is_poisoned());
    /// ```
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        let status = self.lock.load(Ordering::Relaxed);
        is_poisoned(status)
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Note that this method won't acquire any lock because we know there is
    /// no other references to `self`.
    ///
    /// # Errors
    ///
    /// If another user panicked while holding the exclusive write lock of this instance,
    /// this method call wraps the guard in an error and returns it.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::RwLock;
    ///
    /// let mut lock = RwLock::new(0);
    /// *lock.get_mut().unwrap() = 10;
    /// assert_eq!(*lock.read().unwrap(), 10);
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        // We know statically that there are no other references to `self`, so
        // there's no need to lock the inner lock.
        let data = unsafe { &mut *self.data.get() };
        if self.is_poisoned() {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }
}

impl<T> From<T> for RwLock<T> {
    fn from(t: T) -> Self {
        RwLock::new(t)
    }
}

impl<T: Default> Default for RwLock<T> {
    fn default() -> Self {
        RwLock::new(T::default())
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_read() {
            Ok(guard) => f.debug_struct("RwLock").field("data", &&*guard).finish(),
            Err(TryLockError::Poisoned(err)) => f
                .debug_struct("RwLock")
                .field("data", &&**err.get_ref())
                .finish(),
            Err(TryLockError::WouldBlock) => {
                struct LockedPlaceholder;
                impl fmt::Debug for LockedPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<locked>")
                    }
                }

                f.debug_struct("RwLock")
                    .field("data", &LockedPlaceholder)
                    .finish()
            }
        }
    }
}

/// An RAII implementation of a "scoped shared read lock" of a RwLock.
///
/// When this instance is dropped (falls out of scope), the lock will be released.
///
/// The data protected by the RwLock can be accessed to read
/// through this guard via its `Deref` implementation.
///
/// This instance is created by [`read`] and [`try_read`] methods on
/// [`RwLock`].
///
/// [`read`]: struct.RwLock.html#method.read
/// [`try_read`]: struct.RwLock.html#method.try_read
/// [`RwLock`]: struct.RwLock.html
pub struct RwLockReadGuard<'a, T: ?Sized + 'a> {
    rwlock: &'a RwLock<T>,
    _phantom: PhantomNotSend, // To implement !Send.
}

impl<'a, T: ?Sized> RwLockReadGuard<'a, T> {
    #[must_use]
    #[inline]
    fn new(rwlock: &'a RwLock<T>) -> Self {
        Self {
            rwlock,
            _phantom: PhantomNotSend::default(),
        }
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    /// Make sure to release the shared read lock.
    /// This function will never poison the rwlock.
    #[inline]
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

impl<T: ?Sized + fmt::Display> fmt::Display for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: fmt::Debug> fmt::Debug for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwLockReadGuard")
            .field("lock", &self.rwlock)
            .finish()
    }
}

/// An RAII implementation of a "scoped exclusive write lock" of a RwLock.
///
/// When this instance is dropped (falls out of scope), the lock will be released.
///
/// The data protected by the RwLock can be accessed to read/write
/// through this guard via its `Deref` and `DerefMut` implementation.
///
/// This instance is created by [`write`] and [`try_write`] methods on
/// [`RwLock`].
///
/// [`write`]: struct.RwLock.html#method.write
/// [`try_write`]: struct.RwLock.html#method.try_write
/// [`RwLock`]: struct.RwLock.html
pub struct RwLockWriteGuard<'a, T: ?Sized + 'a> {
    rwlock: &'a RwLock<T>,
    _phantom: PhantomNotSend, // To implement !Send.
}

impl<'a, T: ?Sized> RwLockWriteGuard<'a, T> {
    #[must_use]
    #[inline]
    fn new(rwlock: &'a RwLock<T>) -> Self {
        Self {
            rwlock,
            _phantom: PhantomNotSend::default(),
        }
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.rwlock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    /// Make sure to release the exclusive write lock.
    ///
    /// If this user panicked, poison the lock.
    #[inline]
    fn drop(&mut self) {
        let old_status = self.rwlock.lock.load(Ordering::Relaxed);

        let mut new_status = release_exclusive_lock(old_status);
        if std::thread::panicking() {
            new_status = set_poison_flag(new_status);
        }

        self.rwlock.lock.store(new_status, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: fmt::Debug> fmt::Debug for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwLockWriteGuard")
            .field("lock", &self.rwlock)
            .finish()
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
unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}

//
// Helpers for Lock State
//
type LockStatus = u64;

const INIT: LockStatus = 0;
const SHARED_LOCK_MASK: LockStatus = 0x3fffffffffffffff;
const EXCLUSIVE_LOCK_FLAG: LockStatus = 0x4000000000000000;
const POISON_FLAG: LockStatus = 0x8000000000000000;

#[must_use]
#[inline]
fn is_poisoned(s: LockStatus) -> bool {
    (s & POISON_FLAG) != 0
}

#[must_use]
#[inline]
fn set_poison_flag(s: LockStatus) -> LockStatus {
    s | POISON_FLAG
}

#[must_use]
#[inline]
fn is_locked(s: LockStatus) -> bool {
    s & (!POISON_FLAG) != 0
}

#[must_use]
#[inline]
fn is_locked_exclusively(s: LockStatus) -> bool {
    let ret = (s & EXCLUSIVE_LOCK_FLAG) != 0;

    if ret {
        debug_assert_eq!(0, s & SHARED_LOCK_MASK);
    }

    ret
}

#[must_use]
#[inline]
fn acquire_exclusive_lock(s: LockStatus) -> LockStatus {
    debug_assert_eq!(false, is_locked(s));
    s | EXCLUSIVE_LOCK_FLAG
}

#[must_use]
#[inline]
fn release_exclusive_lock(s: LockStatus) -> LockStatus {
    debug_assert_eq!(true, is_locked_exclusively(s));
    s & (!EXCLUSIVE_LOCK_FLAG)
}

#[must_use]
#[inline]
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
#[inline]
fn acquire_shared_lock(s: LockStatus) -> LockStatus {
    debug_assert_eq!(false, is_locked_exclusively(s));

    if count_shared_locks(s) == SHARED_LOCK_MASK {
        panic!("rwlock maximum reader count exceeded");
    }

    s + 1
}

#[must_use]
#[inline]
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
