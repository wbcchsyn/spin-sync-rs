#![feature(optin_builtin_traits)]

mod mutex;
mod result;
mod rwlock;

pub use crate::mutex::{Mutex, MutexGuard};
pub use crate::result::{LockResult, PoisonError, TryLockError, TryLockResult};
pub use crate::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
