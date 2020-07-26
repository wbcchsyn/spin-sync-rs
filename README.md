[![CircleCI](https://circleci.com/gh/wbcchsyn/spin-sync-rs.svg?style=svg)](https://circleci.com/gh/wbcchsyn/spin-sync-rs)
[![Build Status](https://travis-ci.org/wbcchsyn/spin-sync-rs.svg?branch=master)](https://travis-ci.org/wbcchsyn/spin-sync-rs)

spin-sync is a module providing synchronization primitives using spinlock. ([Wikipedia Spinlock](https://en.wikipedia.org/wiki/Spinlock))

The main features are as follows.

* Declaring public structs `Mutex` , `RwLock` , and `Once` . The interfaces are resembles those of `std::sync` .
* Ensuring safety as much as `std::sync` , including poisoning strategy and marker traits.
* Unlike to `std::sync`, functions `Mutex::new` and `RwLock::new` are const; i.e. it is possible to declare
  static `Mutex<T>` and static `RwLock<T>` variables as long as T can be constructed statically.

## How to use

1. Add the following line in dependencies section in your Cargo.toml.

   ```Cargo.toml
   spin-sync = "0.1.2"
   ```

1. Build, test and run your project.

   ```shell
   cargo build
   cargo test
   cargo run
   ```
