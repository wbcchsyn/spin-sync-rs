use std::cell::UnsafeCell;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::{AtomicU8, Ordering};

// Aliases to the return type defined in std::sync.
pub use std::sync::{LockResult, PoisonError, TryLockError, TryLockResult};

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

impl<T> Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    pub fn new(t: T) -> Self {
        Mutex {
            lock: AtomicU8::new(UNLOCKED),
            data: UnsafeCell::new(t),
        }
    }

    /// Consumes this mutex and returns the underlying data.
    ///
    /// Since this call borrows the mutable reference, no lock is needed.
    ///
    /// # Errors
    ///
    /// If another user panicked whild holding this mutex, this function call
    /// will wrap the result in a error and return it.
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
        // There is no other references to `self` because the argument is
        // a mutable reference.
        // No lock is required.
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
    /// Acquires a mutex, blocking the current thread until it is able to do so.
    ///
    /// This function will block the local thread until it is available to acquire
    /// the mutex. Upon returning, the thread is the only thread with the lock
    /// held. An RAII guard is returned to allow scoped unlock of the lock. When
    /// the guard goes out of scope, the mutex will be unlocked.
    ///
    /// # Errors
    ///
    /// If another user panicked while holding this mutex, this method call wraps
    /// the guard in an error and return it.
    ///
    /// # Example
    ///
    /// ```
    ///    use spin_sync::Mutex;
    ///    use std::sync::Arc;
    ///    use std::thread;
    ///
    ///    const NUM: usize = 10;
    ///
    ///    let mutex = Arc::new(Mutex::new(0));
    ///    let mut handles = Vec::with_capacity(NUM);
    ///
    ///    for _ in 0..NUM {
    ///        let mutex = mutex.clone();
    ///        let handle = thread::spawn(move || {
    ///            let mut num = mutex.lock().unwrap();
    ///            *num += 1;
    ///        });
    ///        handles.push(handle);
    ///    }
    ///
    ///    for handle in handles {
    ///        handle.join().unwrap();
    ///    }
    ///
    ///    assert_eq!(NUM, *mutex.lock().unwrap());
    /// ```
    pub fn lock(&self) -> LockResult<MutexGuard<T>> {
        // Assume this mutex is not poisoned and try to lock.
        loop {
            match self
                .lock
                .compare_and_swap(UNLOCKED, LOCKED, Ordering::Acquire)
            {
                UNLOCKED => return Ok(MutexGuard::new(self)), // succeeded
                LOCKED => std::thread::yield_now(),           // locked
                _ => break,                                   // poisoned
            }
        }

        // This mutex is found to be poisoned.
        // Try to lock again.
        loop {
            match self
                .lock
                .compare_and_swap(POISON_UNLOCKED, POISON_LOCKED, Ordering::Acquire)
            {
                POISON_UNLOCKED => return Err(PoisonError::new(MutexGuard::new(self))),
                POISON_LOCKED => std::thread::yield_now(),
                _ => panic!("Bag! program should not come here."),
            }
        }
    }

    /// Attempt to acquire this lock.
    ///
    /// If another user is holding this lock, return an error immediately;
    /// otherwise, return a RAII guard.
    ///
    /// This function does not block.
    ///
    /// # Success
    ///
    /// If this function call succeeded to acquire this lock, and if no user has
    /// never panicked while holding this mutex, this function call success and
    /// return a RAII guard.
    ///
    /// # Errors
    ///
    /// - If another user is holding this mutex, return TryLockError::WouldBlock.
    /// - If this function call succeeded to acquire the lock, and if another
    ///   user paniced while holding this mutex, return
    ///   TryLockError::Poisoned(PoisonError<MutexGuard<T>>)
    ///
    /// # Examples
    ///
    /// ```
    ///    use spin_sync::{Mutex, TryLockError};
    ///    use std::sync::Arc;
    ///    use std::thread;
    ///
    ///    let mutex = Arc::new(Mutex::new(0));
    ///
    ///    // Create a new thread to call mutex.try_to_lock() while holding
    ///    // the mutex in main thread.
    ///    {
    ///        let guard = mutex.lock().unwrap();
    ///        let mutex = mutex.clone();
    ///
    ///        thread::spawn(move || {
    ///            match mutex.try_lock() {
    ///                Err(TryLockError::WouldBlock) => assert!(true),
    ///                _ => assert!(false),
    ///            }
    ///        }).join().unwrap();
    ///    }
    ///
    ///    // Release the lock and try to increment the mutex value in another
    ///    // thread.
    ///    {
    ///        let mutex = mutex.clone();
    ///
    ///        thread::spawn(move || {
    ///            let mut num = mutex.try_lock().unwrap();
    ///            *num += 1;
    ///        }).join().unwrap();
    ///    }
    ///
    ///    assert_eq!(1, *mutex.try_lock().unwrap());
    /// ```
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<T>> {
        // Assume this mutex is not poisoned and try to lock.
        match self
            .lock
            .compare_and_swap(UNLOCKED, LOCKED, Ordering::Acquire)
        {
            UNLOCKED => return Ok(MutexGuard::new(self)), // succeeded
            s if is_locked(s) => return Err(TryLockError::WouldBlock), // locked,
            _ => (),                                      // poisoned
        }

        // This mutex was poisoned. Try again.
        match self
            .lock
            .compare_and_swap(POISON_UNLOCKED, POISON_LOCKED, Ordering::Acquire)
        {
            POISON_LOCKED => Err(TryLockError::WouldBlock),
            POISON_UNLOCKED => Err(TryLockError::Poisoned(PoisonError::new(MutexGuard::new(
                self,
            )))),
            _ => panic!("Bag!, program should not come here."),
        }
    }

    /// Determin whether the mutex is poisoned or not.
    ///
    /// # Warning
    ///
    /// This function won't acquire this lock. If another thread is active,
    /// the mutex can become poisoned at any time. You should not trust a `false`
    /// value for program correctness without additional synchronization.
    ///
    /// This behavior is same to std::sync::Mutex::is_poisoned().
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
    /// {
    ///     let mutex = mutex.clone();
    ///
    ///     let _ = thread::spawn(move || {
    ///         let _guard = mutex.lock().unwrap();
    ///         panic!("Poison this mutex");
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
    /// Since this call borrows the mutable reference, no lock is needed.
    ///
    /// # Errors
    ///
    /// If another user panicked whild holding this mutex, this function call
    /// will wrap the result in a error and return it.
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
/// When this structure is dropped (falls out of scope), the lock will be
/// unlocked.
///
/// The data protected by the mutex can be accessed through this guard via its
/// Deref and DerefMut implementations.
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a Mutex<T>,
}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    fn new(mutex: &'a Mutex<T>) -> Self {
        Self { mutex }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        let old_status = self.mutex.lock.load(Ordering::Relaxed);
        debug_assert!(is_locked(old_status));

        let new_status = if is_poisoned(old_status) || std::thread::panicking() {
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

/// Check the status is locked or not.
fn is_locked(s: LockState) -> bool {
    debug_assert!(s <= MAX_LOCK_STATE);
    (s % 2) == 1
}

/// Check the status is poisoned or not.
fn is_poisoned(s: LockState) -> bool {
    debug_assert!(s <= MAX_LOCK_STATE);
    (s / 2) == 1
}
