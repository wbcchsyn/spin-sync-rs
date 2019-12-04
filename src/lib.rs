#![feature(external_doc)]
#![doc(include = "../README.md")]

mod misc;
mod mutex;
mod result;
mod rwlock;

pub use crate::mutex::{Mutex, MutexGuard};
pub use crate::result::{LockResult, PoisonError, TryLockError, TryLockResult};
pub use crate::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
