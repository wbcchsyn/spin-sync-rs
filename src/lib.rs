#![feature(external_doc)]

//! [![CircleCI](https://circleci.com/gh/wbcchsyn/spin-sync-rs.svg?style=svg)](https://circleci.com/gh/wbcchsyn/spin-sync-rs)
//! [![Build Status](https://travis-ci.org/wbcchsyn/spin-sync-rs.svg?branch=master)](https://travis-ci.org/wbcchsyn/spin-sync-rs)
//!
//! spin-sync is a module providing synchronization primitives using spinlock. ([Wikipedia Spinlock](https://en.wikipedia.org/wiki/Spinlock))
//!
//! The main features are as follows.
//!
//! - Declaring public structs `Mutex` and `RwLock`, whose interfaces are resembles those of `std::sync`.
//! - Ensuring safety as much as `std::sync`.
//! - Unfortunately, rust nightly version is required so far.
//!
//! ## How to use
//!
//! 1. Add the following line in dependencies section in your Cargo.toml.
//!
//!    ```Cargo.toml
//!    spin-sync = "0.0.1"
//!    ```
//!
//! 1. Make sure to install rust nightly toolchain.
//!
//!    ```shell
//!    rustup toolchain install nightly
//!    ```
//!
//! 1. Build, test and run your project.
//!
//!    ```shell
//!    cargo +nightly build
//!    cargo +nightly test
//!    cargo +nightly run
//!    ```
//!
//! ## Why nightly toolchain is required?
//!
//! To implement negative trait `!Sync`.
//!
//! It is necessary to enable the rust compiler to find a kind of bug. (std::sync do the same thing, too.) Rust compiler requests the feature `option_builtin_traits` and the nightly toolchain to do it except for the std library.
//!
//! ## Examples
//!
//! ### Mutex<T>
//!
//! `Mutex::lock()` acquires the exclusive lock and returns an RAII guard object. The lock will be released when the guard is dropped (falls out of scope.)
//!
//! The data protected by the mutex can be accessed through this guard via its `Defer` and `DeferMut` implementations.
//!
//! ```
//! extern crate spin_sync;
//!
//! use spin_sync::Mutex;
//! use std::sync::Arc;
//! use std::thread;
//!
//! /// Create a variable protected by a Mutex, increment it in worker threads,
//! /// and check the variable was updated rightly.
//! fn main() {
//!     const WORKER_NUM: usize = 10;
//!     let mut handles = Vec::with_capacity(WORKER_NUM);
//!
//!     // Decrare a variable protected by Mutex.
//!     // It is wrapped in std::Arc to share this mutex itself among threads.
//!     let mutex = Arc::new(Mutex::new(0));
//!
//!     // Create worker threads to inclement the value by 1.
//!     for _ in 0..WORKER_NUM {
//!         let mutex = mutex.clone();
//!
//!         let handle = thread::spawn(move || {
//!             let mut num = mutex.lock().unwrap();
//!             *num += 1;
//!         });
//!
//!         handles.push(handle);
//!     }
//!
//!     // Wait for the all worker threads are finished.
//!     for handle in handles {
//!         handle.join().unwrap();
//!     }
//!
//!     // Make sure the value is incremented by the worker count.
//!     let num = mutex.lock().unwrap();
//!     assert_eq!(WORKER_NUM, *num);
//! }
//! ```
//!
//! ### RwLock<T>
//!
//! `RwLock` resembles `Mutex` except for it distinguishes readers and writers.
//!
//! `RwLock::write()` behaves like `Mutex::lock()`.
//! It acquires the exclusive write lock and returns an RAII guard object. The lock will be released when the guard is dropped (falls out of scope.)
//! This guard allows read/write access (exclusive access) to the underlying data via its `Defer` and `DeferMut` implementations.
//!
//! `RwLock::read()` behaves like `RwLock::write()` except for it acquires a shared read lock
//! (i.e. this method allows any number of readers to hold a shared read lock at the same time as long as no writer is not holding the exclusive write lock.)
//! This guard allows read-only access (shared access) to the underlying data via its `Defer` implementation.
//!
//! ```
//! extern crate spin_sync;
//!
//! use spin_sync::RwLock;
//! use std::sync::Arc;
//! use std::thread;
//!
//! /// Create a variable protected by a RwLock, increment it by 2 in worker threads,
//! /// and check the variable was updated rightly.
//! fn main() {
//!     const WORKER_NUM: usize = 10;
//!     let mut handles = Vec::with_capacity(WORKER_NUM);
//!
//!     // Decrare a variable protected by RwLock.
//!     // It is wrapped in std::Arc to share this instance itself among threads.
//!     let rwlock = Arc::new(RwLock::new(0));
//!
//!     // Create worker threads to inclement the value by 2.
//!     for _ in 0..WORKER_NUM {
//!         let c_rwlock = rwlock.clone();
//!
//!         let handle = thread::spawn(move || {
//!             let mut num = c_rwlock.write().unwrap();
//!             *num += 2;
//!         });
//!
//!         handles.push(handle);
//!     }
//!
//!     // Make sure the value is always multipile of 2 even if some worker threads
//!     // are working (it is incremented by 2.)
//!     //
//!     // Enclosing the lock with `{}` to drop it before waiting for the worker
//!     // threads; otherwise, deadlocks could be occurred.
//!     {
//!         let num = rwlock.read().unwrap();
//!         assert_eq!(0, *num % 2);
//!     }
//!
//!     // Wait for the all worker threads are finished.
//!     for handle in handles {
//!         handle.join().unwrap();
//!     }
//!
//!     // Make sure the value is incremented by 2 times the worker count.
//!     let num = rwlock.read().unwrap();
//!     assert_eq!(2 * WORKER_NUM, *num);
//! }
//! ```

mod misc;
mod mutex;
mod result;
mod rwlock;

pub use crate::mutex::{Mutex, MutexGuard};
pub use crate::result::{LockResult, PoisonError, TryLockError, TryLockResult};
pub use crate::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
