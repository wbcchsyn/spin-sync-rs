#![feature(optin_builtin_traits)]

mod mutex;
mod result;
mod rwlock;

pub use self::mutex::{LockResult, Mutex, MutexGuard, PoisonError, TryLockError, TryLockResult};

pub use self::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
