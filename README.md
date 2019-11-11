[![CircleCI](https://circleci.com/gh/wbcchsyn/spin-sync-rs.svg?style=svg)](https://circleci.com/gh/wbcchsyn/spin-sync-rs)
[![Build Status](https://travis-ci.org/wbcchsyn/spin-sync-rs.svg?branch=master)](https://travis-ci.org/wbcchsyn/spin-sync-rs)

spin-sync is a module providing synchronization primitives using spinlock. ([Wikipedia Spinlock](https://en.wikipedia.org/wiki/Spinlock))

The main features are as follows.

- Declaring public structs `Mutex` and `RwLock`, whose interfaces are resembles those of `std::sync`.
- Ensuring safety as much as `std::sync`.
- Unfortunately, rust nightly version is required so far.

## How to use

1. Add the following line in dependencies section in your Cargo.toml.

   `spin-sync = "0.0.1"`

1. Make sure to install rust nightly toolchain.

   ```shell
   rustup toolchain install nightly
   ```

1. Build, test and run your project.

   ```shell
   cargo +nightly build
   cargo +nightly test
   cargo +nightly run
   ```

## Why nightly toolchain is required?

To implement negative trait `!Sync`.

It is necessary to enable the rust compiler to find a kind of bug. (std::sync do the same thing, too.) Rust compiler requests the feature `option_builtin_traits` and the nightly toolchain to do it except for the std library.

## Examples

### Mutex<T>

`Mutex::lock()` acquires the exclusive lock and returns an RAII guard object. The lock will be released when the guard is dropped (`drop(guard)` is called explicitly, or it falls out of scope.)

The data protected by the mutex can be accessed through this guard via its Defer and DeferMut implementations.

```
extern crate spin_sync;

use spin_sync::Mutex;
use std::sync::Arc;
use std::thread;

fn main() {
    const THREAD_NUM: usize = 10;
    let mut handles = Vec::new();

    // Decrare a variable which is i32 protected by Mutex.
    // It is wrapped by std::Arc to share this mutex itself among many threads.
    let mutex = Arc::new(Mutex::new(0));

    for _ in 0..THREAD_NUM {
        let c_mutex = mutex.clone();

        let handle = thread::spawn(move || {
            let mut num = c_mutex.lock().unwrap();
            *num += 1;
        });

        handles.push(handle);
    }

    // Wait for the all threads are finished.
    for handle in handles {
        handle.join().unwrap();
    }

    // Make sure the value is incremented by the thread count.
    let num = mutex.lock().unwrap();
    assert_eq!(THREAD_NUM, *num);
}
```

### RwLock<T>

`RwLock` resembles `Mutex` except for it distinguishes readers and writers.

`RwLock::write()` behaves like `Mutex::lock()`.
It acquires the exclusive lock and returns an RAII guard object. The lock will be released when the object is dropped (`drop(guard)` is called explicitly, or it falls out of scope.)
The data protected by the rwlock can be accessed through this guard via its Defer and DeferMut implementations.

`RwLock::read()` behaves like `RwLock::write()` except for it acquires a shared read lock (i.e. this method allows any number of readers to hold a shared read lock at the same time as long as no writer is not holding the exclusive write lock.)
The data protected by the rwlock can be accesssed through the guard via its Defer implementation.

```
extern crate spin_sync;

use spin_sync::RwLock;
use std::sync::Arc;
use std::thread;


/// Create an i32 variable protected by a RwLock, increment it by 2 in many threads at the same time,
/// and check the variable was updated rightly.
fn main() {
    const THREAD_NUM: usize = 10;
    let mut handles = Vec::new();

    // Decrare a variable which is i32 protected by RwLock.
    // It is wrapped by std::Arc to share this RwLock itself among many threads.
    let rwlock = Arc::new(RwLock::new(0));


    // Create threads to inclement the value by 2 and store them into a vector.
    for _ in 0..THREAD_NUM {
        let c_rwlock = rwlock.clone();

        let handle = thread::spawn(move || {
            let mut num = c_rwlock.write().unwrap();
            *num += 2;
        });

        handles.push(handle);
    }

    // Make sure the value is always multipile of 2 even if some threads are working,
    // because it is incremented by 2.
    //
    // Enclosing the assertion in `{}` to release the shared read lock before joining the threads;
    // otherwise, some deadlocks could be occurred.
    {
        let num = rwlock.read().unwrap();
        assert_eq!(0, *num % 2);
    }

    // Wait for the all threads are finished.
    for handle in handles {
        handle.join().unwrap();
    }

    // Make sure the value is incremented by the thread count.
    let num = rwlock.read().unwrap();
    assert_eq!(2 * THREAD_NUM, *num);
}
```
