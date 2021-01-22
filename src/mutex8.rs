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
use std::sync::atomic::{AtomicU8, Ordering};

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
}
