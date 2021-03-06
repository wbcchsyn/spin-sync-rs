// Copyright 2020 Shin Yoshida
//
// "LGPL-3.0-or-later OR Apache-2.0 OR BSD-2-Clause"
//
// This is part of spin-sync
//
//  spin-sync is free software: you can redistribute it and/or modify
//  it under the terms of the GNU Lesser General Public License as published by
//  the Free Software Foundation, either version 3 of the License, or
//  (at your option) any later version.
//
//  spin-sync is distributed in the hope that it will be useful,
//  but WITHOUT ANY WARRANTY; without even the implied warranty of
//  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//  GNU Lesser General Public License for more details.
//
//  You should have received a copy of the GNU Lesser General Public License
//  along with spin-sync.  If not, see <http://www.gnu.org/licenses/>.
//
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
//
// Redistribution and use in source and binary forms, with or without modification, are permitted
// provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of
//    conditions and the following disclaimer.
// 2. Redistributions in binary form must reproduce the above copyright notice, this
//    list of conditions and the following disclaimer in the documentation and/or other
//    materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
// ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
// IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
// INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
// NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
// PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
// WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.

use std::cell::UnsafeCell;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::{AtomicU8, Ordering};

use crate::misc::{PhantomMutex, PhantomMutexGuard};
use crate::result::{LockResult, PoisonError, TryLockError, TryLockResult};

/// A mutual exclusion primitive useful for protecting shared data.
///
/// It behaves like std::sync::Mutex except for using spinlock.
/// What is more, the constructor is a const function; i.e. it is possible to declare
/// static Mutex<T> variable as long as the inner data can be built statically.
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
/// use std::thread;
///
/// const WORKER_NUM: usize = 10;
///
/// // We can declare static Mutex<usize> variable because Mutex::new is const.
/// static MUTEX: Mutex<usize> = Mutex::new(0);
///
/// let mut handles = Vec::with_capacity(WORKER_NUM);
///
/// // Create worker threads to inclement the value by 1.
/// for _ in 0..WORKER_NUM {
///     let handle = thread::spawn(move || {
///         let mut num = MUTEX.lock().unwrap();
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
/// let num = MUTEX.lock().unwrap();
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
/// // Like std::sync::Mutex, it can be declare as local variable, of course.
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
    _phantom: PhantomMutex<T>,
    data: UnsafeCell<T>,
}

impl<T> Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    ///
    /// unlike to `std::sync::Mutex::new`, this is a const function.
    /// It can be use for static variable.
    ///
    /// # Examples
    ///
    /// Declare as a static variable.
    ///
    /// ```
    /// use spin_sync::Mutex;
    ///
    /// static MUTEX: Mutex<i32> = Mutex::new(0);
    /// ```
    ///
    /// Declare as a local variable.
    ///
    /// ```
    /// use spin_sync::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    /// ```
    pub const fn new(t: T) -> Self {
        Mutex {
            lock: AtomicU8::new(INIT),
            data: UnsafeCell::new(t),
            _phantom: PhantomMutex {},
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
#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a Mutex<T>,
    _phantom: PhantomMutexGuard<'a, T>, // To implement !Send.
}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    fn new(mutex: &'a Mutex<T>) -> Self {
        Self {
            mutex,
            _phantom: Default::default(),
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
