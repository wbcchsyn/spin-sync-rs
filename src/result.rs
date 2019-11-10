use std::sync;

/// Alias to std::sync::LockResult.
pub type LockResult<T> = sync::LockResult<T>;

/// Alias to std::sync::PoisonError
pub type PoisonError<T> = sync::PoisonError<T>;

/// Alias to std::sync::TryLockError
pub type TryLockError<T> = sync::TryLockError<T>;

/// Alias to std::sync::TryLockResult
pub type TryLockResult<T> = sync::TryLockResult<T>;
