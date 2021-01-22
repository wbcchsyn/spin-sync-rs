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

//! [![CircleCI](https://circleci.com/gh/wbcchsyn/spin-sync-rs.svg?style=svg)](https://circleci.com/gh/wbcchsyn/spin-sync-rs)
//! [![Build Status](https://travis-ci.org/wbcchsyn/spin-sync-rs.svg?branch=master)](https://travis-ci.org/wbcchsyn/spin-sync-rs)
//!
//! spin-sync is a module providing synchronization primitives using spinlock. ([Wikipedia Spinlock](https://en.wikipedia.org/wiki/Spinlock))
//!
//! The main features are as follows.
//!
//! * Declaring public structs `Mutex` , `RwLock` , `Once` , `Barrier` . The interfaces are resembles those of `std::sync` .
//! * Ensuring safety as much as `std::sync` , including poisoning strategy and marker traits.
//! * Unlike to `std::sync`, the constructors of the public structs are const; i.e. it is possible to declare
//!   static `Mutex<T>` as long as T can be build statically.
//!
//! ## How to use
//!
//! 1. Add the following line in dependencies section in your Cargo.toml.
//!
//!    ```Cargo.toml
//!    spin-sync = "0.2.1"
//!    ```
//!
//! 1. Build, test and run your project.
//!
//!    ```shell
//!    cargo build
//!    cargo test
//!    cargo run
//!    ```
//!
//! ## Examples
//!
//! Declare `static spin_sync::Mutex<u64>` variable and update from multi threads.
//! It is impossible in case of `std::sync::Mutex` .
//!
//! ```
//! extern crate spin_sync;
//!
//! use spin_sync::Mutex;
//! use std::thread;
//!
//! // Declare static mut Mutex<u64> variable.
//! static COUNT: Mutex<u64> = Mutex::new(0);
//!
//! fn main() {
//!     let num_thread = 10;
//!     let mut handles = Vec::new();
//!     
//!     // Create worker threads to inclement COUNT by 1.
//!     for _ in 0..10 {
//!         let handle = thread::spawn(move || {
//!             let mut count = COUNT.lock().unwrap();
//!             *count += 1;
//!         });
//!
//!         handles.push(handle);
//!     }
//!
//!     // Wait for all the workers.
//!     for handle in handles {
//!         handle.join().unwrap();
//!     }
//!
//!     // Make sure the value is incremented by the worker count.
//!     let count = COUNT.lock().unwrap();
//!     assert_eq!(num_thread, *count);
//! }
//! ```

mod barrier;
mod misc;
mod mutex;
mod mutex8;
mod once;
mod result;
mod rwlock;

pub use crate::barrier::{Barrier, BarrierWaitResult};
pub use crate::mutex::{Mutex, MutexGuard};
pub use crate::mutex8::Mutex8;
pub use crate::once::{Once, OnceState};
pub use crate::result::{LockResult, PoisonError, TryLockError, TryLockResult};
pub use crate::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
