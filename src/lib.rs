#![feature(optin_builtin_traits)]

mod mutex;

pub use self::mutex::{LockResult, Mutex, MutexGuard, PoisonError, TryLockError, TryLockResult};
