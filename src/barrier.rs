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

use crate::misc::{PhantomBarrier, PhantomBarrierWaitResult};
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A barrier enables multiple threads to synchronize the beginning
/// of some computation.
///
/// The behavior is same to `std::sync::Barrier` except for this uses spinlock.
///
/// Unlike to `std::sync::Barrier::new`, the constructor [`Barrier.new`] is a const function;
/// i.e. static [`Barrier`] variable can be declared.
///
/// [`Barrier`]: struct.Barrier.html
/// [`Barrier.new`]: #method.new
///
/// # Examples
///
/// ```
/// use spin_sync::Barrier;
/// use std::thread;
///
/// static NUM_THREADS: usize = 10;
/// static BARRIER: Barrier = Barrier::new(NUM_THREADS);
///
/// let mut handles = Vec::with_capacity(10);
/// for _ in 0..10 {
///     // The same messages will be printed together.
///     // You will NOT see any interleaving.
///     handles.push(thread::spawn(move|| {
///         println!("before wait");
///         BARRIER.wait();
///         println!("after wait");
///     }));
/// }
/// // Wait for other threads to finish.
/// for handle in handles {
///     handle.join().unwrap();
/// }
/// ```
///
/// Once all the threads have finished to wait, `Barrier` is reinitialized.
/// The same instance can be used again.
///
/// ```
/// use spin_sync::Barrier;
/// use std::thread;
///
/// static NUM_THREADS: usize = 10;
/// static BARRIER: Barrier = Barrier::new(NUM_THREADS);
///
/// fn wait_and_reinitialize() {
///     let mut handles = Vec::with_capacity(10);
///     for _ in 0..10 {
///         // The same messages will be printed together.
///         // You will NOT see any interleaving.
///         handles.push(thread::spawn(move|| {
///             println!("before wait");
///             BARRIER.wait();
///             println!("after wait");
///         }));
///     }
///     // Wait for other threads to finish.
///     for handle in handles {
///         handle.join().unwrap();
///     }
/// }
///
/// fn main() {
///     // First use.
///     wait_and_reinitialize();
///     // Second use.
///     wait_and_reinitialize();
/// }
/// ```
///
/// If 0 or 1 is passed to `Barrier::new`, the instance will never block.
///
/// ```
/// use spin_sync::Barrier;
/// use std::thread;
///
/// static BARRIER0: Barrier = Barrier::new(0);
/// static BARRIER1: Barrier = Barrier::new(1);
///
/// BARRIER0.wait();
/// BARRIER1.wait();
/// ```
pub struct Barrier {
    num_threads: usize, // immutable
    count: AtomicUsize,
    generation_id: AtomicUsize, // MSB plays lock flag role.
    _phantom: PhantomBarrier,
}

impl Barrier {
    /// Creates a new barrier that can block a given number of threads.
    ///
    /// Unlike to `std::sync::Barrier::new`, this function is const; i.e.
    /// static [`Barrier`] variable can be declared.
    ///
    /// A barrier will block `n`-1 threads which call [`wait`] and then wake up
    /// all threads at once when the `n`th thread calls [`wait`].
    ///
    /// [`Barrier`]: struct.Barrier.html
    /// [`wait`]: #method.wait
    ///
    /// # Examples
    ///
    /// Declaring [`Barrier`] instance as a local variable.
    ///
    /// ```
    /// use spin_sync::Barrier;
    ///
    /// let barrier = Barrier::new(10);
    /// ```
    ///
    /// Declaring static [`Barrier`] variable.
    ///
    /// ```
    /// use spin_sync::Barrier;
    ///
    /// static BARRIER: Barrier = Barrier::new(5);
    /// ```
    pub const fn new(n: usize) -> Self {
        Self {
            num_threads: n,
            count: AtomicUsize::new(0),
            generation_id: AtomicUsize::new(0),
            _phantom: PhantomBarrier {},
        }
    }

    /// Blocks the current thread until all threads have rendezvoused here.
    ///
    /// Barriers are re-usable after all threads have rendezvoused once, and can
    /// be used continuously.
    ///
    /// A single (arbitrary) thread will receive a [`BarrierWaitResult`] that
    /// returns `true` from [`is_leader`] when returning from this function, and
    /// all other threads will receive a result that will return `false` from
    /// [`is_leader`].
    ///
    /// [`BarrierWaitResult`]: struct.BarrierWaitResult.html
    /// [`is_leader`]: struct.BarrierWaitResult.html#method.is_leader
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::Barrier;
    /// use std::thread;
    ///
    /// static NUM_THREADS: usize = 10;
    /// static BARRIER: Barrier = Barrier::new(NUM_THREADS);
    ///
    /// let mut handles = Vec::with_capacity(10);
    /// for _ in 0..10 {
    ///     // The same messages will be printed together.
    ///     // You will NOT see any interleaving.
    ///     handles.push(thread::spawn(move|| {
    ///         println!("before wait");
    ///         BARRIER.wait();
    ///         println!("after wait");
    ///     }));
    /// }
    /// // Wait for other threads to finish.
    /// for handle in handles {
    ///     handle.join().unwrap();
    /// }
    /// ```
    pub fn wait(&self) -> BarrierWaitResult {
        let (guard, generation_id) = self.lock();

        let count = self.count.load(Ordering::Relaxed) + 1;
        self.count.store(count, Ordering::Relaxed);

        if count < self.num_threads {
            // Unlock and waiting for the leader reinitialize self.
            drop(guard);

            loop {
                let mut current_id = self.generation_id.load(Ordering::Relaxed);
                if (current_id & BarrierLockGuard::MSB) != 0 {
                    current_id = current_id - BarrierLockGuard::MSB;
                }

                if generation_id != current_id {
                    return BarrierWaitResult(false, PhantomBarrierWaitResult {});
                } else {
                    std::thread::yield_now();
                }
            }
        } else {
            // This thread will be the leader.
            // Reinitialize self and return immediately.
            self.count.store(0, Ordering::Relaxed);

            // The other waiting threads judge whether reinitialized or not from generation_id.
            // After generation_id was updated, they stop to block and return.
            // However, the next wait() won't be started because this thread still owns the lock.
            let generation_id = (generation_id + 1) | BarrierLockGuard::MSB;
            self.generation_id.store(generation_id, Ordering::Relaxed);

            // Release the lock.
            drop(guard);

            BarrierWaitResult(true, PhantomBarrierWaitResult {})
        }
    }

    fn lock(&self) -> (BarrierLockGuard, usize) {
        // Acquire lock
        let mut expected = 0;
        loop {
            let desired = expected + BarrierLockGuard::MSB;

            let current = self
                .generation_id
                .compare_and_swap(expected, desired, Ordering::Acquire);

            if current == expected {
                // Succeeded to lock
                break;
            } else {
                // Failed to lock.
                // Retry.
                if (current & BarrierLockGuard::MSB) != 0 {
                    // Another thread is holding the lock.
                    // Wait for a while and retry.
                    expected = current - BarrierLockGuard::MSB;
                    std::thread::yield_now();
                } else {
                    // Just the first assumption was wrong.
                    // Retry immediately.
                    expected = current;
                }
            }
        }

        (
            BarrierLockGuard {
                generation_id: &self.generation_id,
            },
            expected,
        )
    }
}

impl fmt::Debug for Barrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Barrier { .. }")
    }
}

pub struct BarrierWaitResult(bool, PhantomBarrierWaitResult);

impl BarrierWaitResult {
    /// Returns `true` if this thread from [`wait`] is the "leader thread".
    ///
    /// Only one thread will have `true` returned from their result, all other
    /// threads will have `false` returned.
    ///
    /// [`wait`]: struct.Barrier.html#method.wait
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Barrier;
    ///
    /// let barrier = Barrier::new(1);
    /// let barrier_wait_result = barrier.wait();
    /// assert!(barrier_wait_result.is_leader());
    /// ```
    pub fn is_leader(&self) -> bool {
        self.0
    }
}

impl fmt::Debug for BarrierWaitResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BarrierWaitResult")
            .field("is_leader", &self.is_leader())
            .finish()
    }
}

struct BarrierLockGuard<'a> {
    generation_id: &'a AtomicUsize,
}

impl BarrierLockGuard<'_> {
    pub const MSB: usize = usize::MAX / 2 + 1;
}

impl Drop for BarrierLockGuard<'_> {
    // Make sure to unlock
    fn drop(&mut self) {
        let current = self.generation_id.load(Ordering::Relaxed);
        debug_assert_eq!(Self::MSB, current & Self::MSB);

        let desired = current - Self::MSB;
        self.generation_id.store(desired, Ordering::Release);
    }
}
