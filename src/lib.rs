#![feature(optin_builtin_traits)]

pub mod mutex;

pub use self::mutex::{LockResult, Mutex, MutexGuard, PoisonError, TryLockError, TryLockResult};
