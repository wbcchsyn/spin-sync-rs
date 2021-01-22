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

use crate::misc::PhantomMutexGuard;
use crate::result::{TryLockError, TryLockResult};
use std::fmt::{self, Debug, Display};
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;

/// `Mutex8` is a set of mutexes. Each instance includes 8 mutexes.
///
/// The differences between `Mutex8` and [`Mutex`] are as follows.
///
/// - `Mutex8` is not template structure. User must make sure to acquire lock before accessing to
///   the protected object. (Compiler cannot check it.)
/// - `Mutex8` gives up poisoning strategy. (This feature makes the performance better. It is a
///   good idea to use `Mutex8` instead of [`Mutex`] for the performance.)
/// - User can acquire 2 or more than 2 locks of one `Mutex8` instance at once.
///
/// [`Mutex`]: struct.Mutex.html
pub struct Mutex8(AtomicU8);

impl Mutex8 {
    /// Creates a new instance in an unlocked state ready for use.
    ///
    /// Unlike to `std::sync::Mutex` , this is a const function.
    /// It can be use to initialize static variable.
    ///
    /// # Examples
    ///
    /// Declaring a static variable.
    ///
    /// ```
    /// use spin_sync::Mutex8;
    ///
    /// static mutex8: Mutex8 = Mutex8::new();
    /// ```
    ///
    /// Declaring a local variable.
    ///
    /// ```
    /// use spin_sync::Mutex8;
    ///
    /// let mutex8 = Mutex8::new();
    /// ```
    pub const fn new() -> Self {
        Self(AtomicU8::new(0))
    }
}
impl Display for Mutex8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Mutex8")
    }
}

impl Debug for Mutex8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mutex8")
            .field("locked_bits", &self.locked_bits())
            .finish()
    }
}

impl Mutex8 {
    /// Blocks the current thread until acquiring the lock(s) indicated by `lock_bits` and returns
    /// an RAII guard object.
    ///
    /// Each bit of `lock_bits` indicates the lock of `Mutex8` . For example, '0x01' corresponds
    /// to the first lock and '0x02' does to the second lock. If 2 or more than 2 bits are set, the
    /// `lock_bits` means all of them. In case of '0x03', for example, it means both the first and
    /// the second locks.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::Mutex8;
    ///
    /// let mutex8 = Mutex8::new();
    ///
    /// // Acquire '0x01' and '0x02' in order.
    /// {
    ///     let guard1 = mutex8.lock(0x01);
    ///     let guard2 = mutex8.lock(0x02);
    /// }
    ///
    /// // Acquire '0x01' and '0x02' at the same time.
    /// {
    ///     let guard3 = mutex8.lock(0x03);
    /// }
    /// ```
    pub fn lock(&self, lock_bits: u8) -> Mutex8Guard {
        let mut expected = 0;
        while {
            debug_assert_eq!(0, expected & lock_bits);
            let locked = expected + lock_bits;
            let current = self.0.compare_and_swap(expected, locked, Ordering::Acquire);

            if expected == current {
                // Succeeded to acquire
                false
            } else if current & lock_bits == 0 {
                // The first assumuption was wrong.
                // Try again soon.
                expected = current;
                true
            } else {
                // Lock competition.
                thread::yield_now();
                true
            }
        } {}

        Mutex8Guard {
            bits: lock_bits,
            mutex8: &self,
            _phantom: Default::default(),
        }
    }

    /// Attempts to acquire lock(s) indicated by `lock_bits` and returns an RAII guard object if
    /// succeeded.
    ///
    /// Each bit of `lock_bits` indicates the lock of `Mutex8` . For example, '0x01' corresponds
    /// to the first lock and '0x02' does to the second lock. If 2 or more than 2 bits are set, the
    /// `lock_bits` means all of them. In case of '0x03', for example, it means both the first and
    /// the second locks.
    ///
    /// Behaves like [`lock`] except for this method returns an error immediately if another user
    /// is holding the lock.
    ///
    /// This method does not block.
    ///
    /// # Errors
    ///
    /// If another user is holding this mutex, [`TryLockError::WouldBlock`] is returned.
    ///
    /// [`lock`]: #method.lock
    /// [`TryLockError::WouldBlock`]: type.TryLockError.html
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::Mutex8;
    ///
    /// let mutex8 = Mutex8::new();
    ///
    /// // Try to acquire 0x01 twice. The second try will be fail.
    /// {
    ///     let result1 = mutex8.try_lock(0x01);
    ///     assert_eq!(true, result1.is_ok());
    ///
    ///     let result2 = mutex8.try_lock(0x01);
    ///     assert_eq!(true, result2.is_err());
    /// }
    ///
    /// // Try to acquire 0x01 and 0x02 at the same time.
    /// // After that, neither 0x01 nor 0x02 can be locked.
    /// {
    ///     // Acquire locks 0x01 and 0x02 at once.
    ///     let result1 = mutex8.try_lock(0x03);
    ///     assert_eq!(true, result1.is_ok());
    ///
    ///     let result2 = mutex8.try_lock(0x01);
    ///     assert_eq!(true, result2.is_err());
    ///
    ///     let result3 = mutex8.try_lock(0x02);
    ///     assert_eq!(true, result3.is_err());
    /// }
    /// ```
    pub fn try_lock(&self, lock_bits: u8) -> TryLockResult<Mutex8Guard> {
        let mut expected = 0;
        while {
            debug_assert_eq!(0, expected & lock_bits);
            let locked = expected + lock_bits;
            let current = self.0.compare_and_swap(expected, locked, Ordering::Acquire);

            if expected == current {
                // Succeeded to acquire
                false
            } else if current & lock_bits == 0 {
                // The first assumuption was wrong.
                // Try again soon.
                expected = current;
                true
            } else {
                return Err(TryLockError::WouldBlock);
            }
        } {}

        Ok(Mutex8Guard {
            bits: lock_bits,
            mutex8: &self,
            _phantom: Default::default(),
        })
    }

    /// Returns the bits that some [`Mutex8Guard`] instance(s) is holding.
    ///
    /// # Example
    ///
    /// ```
    /// use spin_sync::Mutex8;
    ///
    /// let mutex8 = Mutex8::new();
    ///
    /// // Acquire 0x01.
    /// let guard1 = mutex8.lock(0x01);
    /// assert_eq!(0x01, mutex8.locked_bits());
    ///
    /// // Acquire 0x02.
    /// let guard2 = mutex8.lock(0x02);
    /// assert_eq!(0x03, mutex8.locked_bits());
    ///
    /// // Acquire 0x04 and 0x08 at the same time.
    /// let mut guard3 = mutex8.lock(0x0c);
    /// assert_eq!(0x0f, mutex8.locked_bits());
    ///
    /// // Release 0x08.
    /// guard3.release(0x08);
    /// assert_eq!(0x07, mutex8.locked_bits());
    /// ```
    pub fn locked_bits(&self) -> u8 {
        self.0.load(Ordering::Relaxed)
    }
}

/// An RAII implementation of a "scoped lock(s)" of a [`Mutex8`] .
///
/// When this structure is dropped, all the lock(s) will be released at once.
///
/// [`Mutex8`]: struct.Mutex8.html
#[must_use = "if unused the Mutex8 will immediately unlock"]
pub struct Mutex8Guard<'a> {
    bits: u8,
    mutex8: &'a Mutex8,
    _phantom: PhantomMutexGuard<'a, u8>, // To implement !Send
}

impl Drop for Mutex8Guard<'_> {
    fn drop(&mut self) {
        if self.bits != 0 {
            self.release(self.bits);
        }
    }
}

impl Display for Mutex8Guard<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Mutex8Guard")
    }
}

impl Debug for Mutex8Guard<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mutex8Guard")
            .field("lock_bits", &self.lock_bits())
            .finish()
    }
}

impl Mutex8Guard<'_> {
    /// Releases the lock(s) partially indicated by `lock_bits` .
    ///
    /// Each bit of `lock_bits` indicates the lock of [`Mutex8`] . For example, '0x01' corresponds
    /// to the first lock and '0x02' does to the second lock. If 2 or more than 2 bits are set, the
    /// `lock_bits` means all of them. In case of '0x03', for example, it means both the first and
    /// the second locks.
    ///
    /// If `lock_bits` is same to that is being holded, `self` releases all the locks; otherwise,
    /// the others will still be being holded after the method returns.
    ///
    /// `lock_bits` must not include a bit that `self` is not holding.
    ///
    /// # Panics
    ///
    /// Panics if `lock_bits` includes a bit that `self` is not holding.
    ///
    /// [`Mutex8`]: struct.Mutex8.html
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::Mutex8;
    ///
    /// let mutex8 = Mutex8::new();
    ///
    /// // Acquire 0x01 and 0x02 at the same time.
    /// let mut guard = mutex8.lock(0x03);
    ///
    /// {
    ///     // Fail to acquire 0x01 again.
    ///     let e = mutex8.try_lock(0x01);
    ///     assert!(e.is_err());
    ///
    ///     // Fail to acquire 0x02 again.
    ///     let e = mutex8.try_lock(0x02);
    ///     assert!(e.is_err());
    /// }
    ///
    /// // Release only 0x01. (0x02 is left.)
    /// guard.release(0x01);
    ///
    /// {
    ///     // Success to acquire 0x01 now.
    ///     let o = mutex8.try_lock(0x01);
    ///     assert!(o.is_ok());
    ///
    ///     // Still fail to acquire 0x02.
    ///     let e = mutex8.try_lock(0x02);
    ///     assert!(e.is_err());
    /// }
    /// ```
    pub fn release(&mut self, lock_bits: u8) {
        assert_eq!(lock_bits, self.bits & lock_bits);

        let mut expected = self.bits;
        while {
            debug_assert_eq!(self.bits, expected & self.bits);
            let unlocked = expected - lock_bits;
            let current = self
                .mutex8
                .0
                .compare_and_swap(expected, unlocked, Ordering::Release);

            if current == expected {
                // Succeeded to release.
                self.bits -= lock_bits;
                false
            } else {
                // First assumption was wrong.
                // Try again.
                expected = current;
                true
            }
        } {}
    }

    /// Returns the bits that `self` is holding.
    ///
    /// # Example
    ///
    /// ```
    /// use spin_sync::Mutex8;
    ///
    /// let mutex8 = Mutex8::new();
    ///
    /// // Acquire 0x01 and 0x02 at the same time.
    /// let mut guard = mutex8.lock(0x03);
    /// assert_eq!(0x03, guard.lock_bits());
    ///
    /// // Release only 0x02. (0x01 is left.)
    /// guard.release(0x02);
    /// assert_eq!(0x01, guard.lock_bits());
    ///
    /// // Release 0x01. (No lock is left.)
    /// guard.release(0x01);
    /// assert_eq!(0x00, guard.lock_bits());
    /// ```
    pub fn lock_bits(&self) -> u8 {
        self.bits
    }
}
