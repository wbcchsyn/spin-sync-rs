#![feature(optin_builtin_traits)]

mod mutex;
mod result;
mod rwlock;

pub use self::mutex::{Mutex, MutexGuard};
pub use self::result::{LockResult, PoisonError, TryLockError, TryLockResult};
pub use self::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
