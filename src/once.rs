use crate::misc::PhantomOnce;
use std::sync::atomic::AtomicU8;

/// A synchronization primitive which can be used to run a one-time global initialization.
///
/// `Once` behaves like `std::sync::Once` except for using spinlock.
/// Useful for one-time initialization for FFI or related functionality.
pub struct Once {
    state: AtomicU8,
    _phantom: PhantomOnce,
}

/// State yielded to `call_once_force`’s closure parameter. The state can be used to query
/// the poison status of the `Once`
pub struct OnceState {
    state: u8,
}
