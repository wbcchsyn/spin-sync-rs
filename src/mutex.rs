use std::cell::UnsafeCell;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::{AtomicU8, Ordering};

use crate::misc::PhantomNotSend;
use crate::result::{LockResult, PoisonError, TryLockError, TryLockResult};

/// A mutual exclusion primitive useful for protecting shared data.
///
/// It behaves like std::sync::Mutex except for using spinlock.
///
/// This mutex will block threads waiting for the lock to become available. The
/// mutex can also be statically initialized or created via a [`new`]
/// constructor. Each mutex has a type parameter which represents the data that
/// it is protecting. The data can only be accessed through the RAII guards
/// returned from [`lock`] and [`try_lock`], which guarantees that the data is only
/// ever accessed when the mutex is locked.
///
/// # Poisoning
///
/// The mutexes in this module implement a strategy called "poisoning" where a
/// mutex is considered poisoned whenever a thread panics while holding the
/// mutex. Once a mutex is poisoned, all other threads are unable to access the
/// data by default as it is likely tainted.
///
/// For a mutex, this means that the [`lock`] and [`try_lock`] methods return a
/// `Result` which indicates whether a mutex has been poisoned or not. Most
/// usage of a mutex will simply `unwrap()` these results, propagating panics
/// among threads to ensure that a possibly invalid invariant is not witnessed.
///
/// A poisoned mutex, however, does not prevent all access to the underlying
/// data. The [`PoisonError`] type has an `into_inner` method which will return
/// the guard that would have otherwise been returned on a successful lock. This
/// allows access to the data, despite the lock being poisoned.
///
/// [`new`]: #method.new
/// [`lock`]: #method.lock
/// [`try_lock`]: #method.try_lock
/// [`PoisonError`]: type.PoisonError.html
///
/// # Examples
///
/// Protect a variable (non-atomically) and update it in worker threads.
///
/// ```
/// use spin_sync::Mutex;
/// use std::sync::Arc;
/// use std::thread;
///
/// const WORKER_NUM: usize = 10;
/// let mut handles = Vec::with_capacity(WORKER_NUM);
///
/// // Decrare a variable protected by Mutex.
/// // It is wrapped in std::Arc to share this mutex itself among threads.
/// let mutex = Arc::new(Mutex::new(0));
///
/// // Create worker threads to inclement the value by 1.
/// for _ in 0..WORKER_NUM {
///     let mutex = mutex.clone();
///
///     let handle = thread::spawn(move || {
///         let mut num = mutex.lock().unwrap();
///         *num += 1;
///     });
///
///     handles.push(handle);
/// }
///
/// // Wait for the all worker threads are finished.
/// for handle in handles {
///     handle.join().unwrap();
/// }
///
/// // Make sure the value is incremented by the worker count.
/// let num = mutex.lock().unwrap();
/// assert_eq!(WORKER_NUM, *num);
/// ```
///
/// To recover from a poisoned mutex:
///
/// ```
/// use spin_sync::Mutex;
/// use std::sync::Arc;
/// use std::thread;
///
/// let mutex = Arc::new(Mutex::new(0));
/// let c_mutex = mutex.clone();
///
/// let _ = thread::spawn(move || -> () {
///     // This thread will acquire the mutex first, unwrapping the result of
///     // `lock` because the lock has not been poisoned.
///     let _guard = c_mutex.lock().unwrap();
///
///     // This panic while holding the lock (`_guard` is in scope) will poison
///     // the mutex.
///     panic!();
/// }).join();
///
/// // Here, the mutex has been poisoned.
/// assert_eq!(true, mutex.is_poisoned());
///
/// // The returned result can be pattern matched on to return the underlying
/// // guard on both branches.
/// let mut guard = match mutex.lock() {
///     Ok(guard) => guard,
///     Err(poisoned) => poisoned.into_inner(),
/// };
///
/// *guard += 1;
/// assert_eq!(1, *guard);
/// ```
pub struct Mutex<T: ?Sized> {
    lock: AtomicU8,
    data: UnsafeCell<T>,
}

impl<T> Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    /// # Examples
    ///
    /// ```
    /// use std::sync::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    /// ```
    pub fn new(t: T) -> Self {
        Mutex {
            lock: AtomicU8::new(INIT),
            data: UnsafeCell::new(t),
        }
    }

    /// Consumes this mutex and returns the underlying data.
    ///
    /// Note that this method won't acquire any lock because we know there is
    /// no other references to `self`.
    ///
    /// # Errors
    ///
    /// If another user panicked while holding this mutex, this call wraps
    /// the result in an error and returns it.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    /// assert_eq!(0, mutex.into_inner().unwrap());
    /// ```
    pub fn into_inner(self) -> LockResult<T> {
        let is_err = self.is_poisoned();
        let data = self.data.into_inner();

        if is_err {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Blocks the current thread until acquiring the lock, and returns an RAII guard object.
    ///
    /// The actual flow will be as follows.
    ///
    /// 1. User calls this method.
    ///    1. Blocks until this thread acquires the exclusive lock.
    ///    1. Creates an RAII guard object.
    ///    1. Wraps the guard in `Result` and returns it. If this mutex has been
    ///       poisoned, it is wrapped in an `Err`; otherwise wrapped in a `Ok`.
    /// 1. User accesses to the underlying data through the returned guard.
    ///    (No other thread can access to the data then.)
    /// 1. The guard is dropped (falls out of scope) and the lock is released.
    ///
    /// # Errors
    ///
    /// If another user panicked while holding this mutex, this method call wraps
    /// the guard in an error and returns it.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    ///
    /// let mut guard = mutex.lock().unwrap();
    /// assert_eq!(0, *guard);
    ///
    /// *guard += 1;
    /// assert_eq!(1, *guard);
    ///
    /// assert_eq!(true, mutex.try_lock().is_err());
    /// ```
    pub fn lock(&self) -> LockResult<MutexGuard<T>> {
        loop {
            match self.do_try_lock() {
                s if is_locked(s) => std::thread::yield_now(),
                s if is_poisoned(s) => return Err(PoisonError::new(MutexGuard::new(self))),
                _ => return Ok(MutexGuard::new(self)),
            }
        }
    }

    /// Attempts to acquire this lock and returns an RAII guard object if succeeded.
    ///
    /// Behaves like [`lock`] except for this method returns an error immediately if another
    /// user is holding the lock.
    ///
    /// This method does not block.
    ///
    /// The actual flow will be as follows.
    ///
    /// 1. User calls this method.
    ///    1. Tries to acquire the lock. If failed (i.e. if the lock is being held,)
    ///       returns an error immediately and this flow is finished here.
    ///    1. Creates an RAII guard object.
    ///    1. Wrapps the guard in `Result` and returns it. If this mutex has been
    ///       poisoned, it is wrapped in an `Err`; otherwise wrapped in an `Ok`.
    /// 1. User accesses to the underlying data through the returned guard.
    ///    (No other thread can access to the data then.)
    /// 1. The guard is dropped (falls out of scope) and the lock is released.
    ///
    /// # Errors
    ///
    /// - If another user is holding this mutex, [`TryLockError::WouldBlock`] is returned.
    /// - If this function call succeeded to acquire the lock, and if another
    ///   user panicked while holding this mutex, [`TryLockError::Poisoned`] is returned.
    ///
    /// [`lock`]: #method.lock
    /// [`TryLockError::WouldBlock`]: type.TryLockError.html
    /// [`TryLockError::Poisoned`]: type.TryLockError.html
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    ///
    /// // try_lock() fails while another guard is.
    /// // It doesn't cause a deadlock.
    /// {
    ///     let _guard = mutex.lock().unwrap();
    ///     assert_eq!(true, mutex.try_lock().is_err());
    /// }
    ///
    /// // try_lock() behaves like lock() if no other guard is.
    /// {
    ///     let mut guard = mutex.try_lock().unwrap();
    ///     assert_eq!(true, mutex.try_lock().is_err());
    ///     *guard += 1;
    /// }
    ///
    /// let guard = mutex.try_lock().unwrap();
    /// assert_eq!(1, *guard);
    /// ```
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<T>> {
        match self.do_try_lock() {
            s if is_locked(s) => Err(TryLockError::WouldBlock),
            s if is_poisoned(s) => Err(TryLockError::Poisoned(PoisonError::new(MutexGuard::new(
                self,
            )))),
            _ => Ok(MutexGuard::new(self)),
        }
    }

    /// Tries to acquire lock and returns the lock status before updated.
    fn do_try_lock(&self) -> LockStatus {
        // Assume neither poisoned nor locked at first.
        let mut expected = INIT;

        loop {
            let desired = acquire_lock(expected);
            match self
                .lock
                .compare_and_swap(expected, desired, Ordering::Acquire)
            {
                s if s == expected => return s, // Succeeded
                s if is_locked(s) => return s,  // Another user is holding the lock.
                s => expected = s,              // Assumption was wrong. Try again.
            }
        }
    }

    /// Determines whether the mutex is poisoned or not.
    ///
    /// # Warnings
    ///
    /// This function won't acquire any lock. If another thread is active,
    /// the mutex can become poisoned at any time. You should not trust a `false`
    /// value for program correctness without additional synchronization.
    ///
    /// This behavior is same to `std::sync::Mutex::is_poisoned()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::Mutex;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let mutex = Arc::new(Mutex::new(0));
    /// assert_eq!(false, mutex.is_poisoned());
    ///
    /// // Panic and poison the mutex.
    /// {
    ///     let mutex = mutex.clone();
    ///
    ///     let _ = thread::spawn(move || {
    ///         // This panic while holding the lock (`_guard` is in scope) will poison
    ///         // the mutex.
    ///         let _guard = mutex.lock().unwrap();
    ///         panic!("Poison here");
    ///     }).join();
    /// }
    ///
    /// assert_eq!(true, mutex.is_poisoned());
    /// ```
    pub fn is_poisoned(&self) -> bool {
        // Don't acquire any lock; otherwise, this function will cause
        // a deadlock if the caller thread is holding the lock.
        let status = self.lock.load(Ordering::Relaxed);
        return is_poisoned(status);
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Note that this method won't acquire any lock because we know there is
    /// no other references to `self`.
    ///
    /// # Errors
    ///
    /// If another user panicked while holding this mutex, this method call
    /// wraps the result in an error and returns it.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::Mutex;
    ///
    /// let mut mutex = Mutex::new(0);
    /// *mutex.get_mut().unwrap() = 10;
    /// assert_eq!(10, *mutex.lock().unwrap());
    /// ```
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        // There is no other references to `self` because the argument is
        // a mutable reference.
        // No lock is required.
        let data = unsafe { &mut *self.data.get() };

        if self.is_poisoned() {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock() {
            Ok(guard) => f.debug_struct("Mutex").field("data", &&*guard).finish(),
            Err(TryLockError::Poisoned(err)) => f
                .debug_struct("Mutex")
                .field("data", &&**err.get_ref())
                .finish(),
            Err(TryLockError::WouldBlock) => {
                struct LockedPlaceholder;
                impl fmt::Debug for LockedPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<locked>")
                    }
                }

                f.debug_struct("Mutex")
                    .field("data", &LockedPlaceholder)
                    .finish()
            }
        }
    }
}

impl<T> From<T> for Mutex<T> {
    fn from(t: T) -> Self {
        Mutex::new(t)
    }
}

impl<T: ?Sized + Default> Default for Mutex<T> {
    fn default() -> Self {
        Mutex::new(T::default())
    }
}

/// An RAII implementation of a "scoped lock" of a mutex.
///
/// When this structure is dropped (falls out of scope), the lock will be released.
///
/// The data protected by the mutex can be accessed through this guard via its
/// `Deref` and `DerefMut` implementations.
///
/// This structure is created by [`lock`] and [`try_lock`] methods on
/// [`Mutex`].
///
/// [`lock`]: struct.Mutex.html#method.lock
/// [`try_lock`]: struct.Mutex.html#method.try_lock
/// [`Mutex`]: struct.Mutex.html
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a Mutex<T>,
    _phantom: PhantomNotSend<'a, T>, // To implement !Send.
}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    fn new(mutex: &'a Mutex<T>) -> Self {
        Self {
            mutex,
            _phantom: PhantomNotSend::default(),
        }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        let old_status = self.mutex.lock.load(Ordering::Relaxed);
        debug_assert!(is_locked(old_status));

        let mut new_status = release_lock(old_status);
        if std::thread::panicking() {
            new_status = set_poison_flag(new_status);
        }

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

impl<T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

//
// Marker Traits
//

impl<T: ?Sized> UnwindSafe for Mutex<T> {}
impl<T: ?Sized> RefUnwindSafe for Mutex<T> {}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

//
// Constants to represent lock state
//
type LockStatus = u8;

const INIT: LockStatus = 0;
const LOCK_FLAG: LockStatus = 0x01;
const POISON_FLAG: LockStatus = 0x02;
const NOT_USED_MASK: LockStatus = 0xfc;

#[inline]
#[must_use]
fn is_locked(s: LockStatus) -> bool {
    debug_assert_eq!(0, s & NOT_USED_MASK);
    (s & LOCK_FLAG) != 0
}

#[inline]
#[must_use]
fn acquire_lock(s: LockStatus) -> LockStatus {
    debug_assert_eq!(false, is_locked(s));
    s | LOCK_FLAG
}

#[inline]
#[must_use]
fn release_lock(s: LockStatus) -> LockStatus {
    debug_assert_eq!(true, is_locked(s));
    s & !(LOCK_FLAG)
}

#[inline]
#[must_use]
fn is_poisoned(s: LockStatus) -> bool {
    debug_assert_eq!(0, s & NOT_USED_MASK);
    (s & POISON_FLAG) != 0
}

#[inline]
#[must_use]
fn set_poison_flag(s: LockStatus) -> LockStatus {
    debug_assert_eq!(0, s & NOT_USED_MASK);
    s | POISON_FLAG
}
