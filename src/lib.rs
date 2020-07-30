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
//!    spin-sync = "0.1.2"
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
mod once;
mod result;
mod rwlock;

pub use crate::barrier::{Barrier, BarrierWaitResult};
pub use crate::mutex::{Mutex, MutexGuard};
pub use crate::once::{Once, OnceState};
pub use crate::result::{LockResult, PoisonError, TryLockError, TryLockResult};
pub use crate::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
