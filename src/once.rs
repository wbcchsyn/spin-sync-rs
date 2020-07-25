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

impl Once {
    /// Create a new `Once` instance.
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(OnceState::default().state),
            _phantom: PhantomOnce {},
        }
    }
}

struct OnceGuard<'a> {
    once: &'a Once,
}

/// State yielded to `call_once_force`’s closure parameter. The state can be used to query
/// the poison status of the `Once`
pub struct OnceState {
    state: u8,
}

impl OnceState {
    const INIT: u8 = 0;
    const LOCK: u8 = 1;
    const FINISHED: u8 = 2;
    const POISONED: u8 = 4;

    #[must_use]
    const fn default() -> Self {
        Self { state: Self::INIT }
    }

    #[must_use]
    const fn new(state: u8) -> Self {
        Self { state }
    }

    #[must_use]
    const fn locked(&self) -> bool {
        (self.state & Self::LOCK) != 0
    }

    #[must_use]
    const fn finished(&self) -> bool {
        (self.state & Self::FINISHED) != 0
    }

    #[must_use]
    const fn poisoned(&self) -> bool {
        (self.state & Self::POISONED) != 0
    }

    #[must_use]
    fn acquire_lock(&self) -> Self {
        debug_assert!(!self.locked());
        Self::new(self.state | Self::LOCK)
    }

    #[must_use]
    fn release_lock(&self) -> Self {
        debug_assert!(self.locked());
        Self::new(self.state ^ Self::LOCK)
    }

    #[must_use]
    fn finish(&self) -> Self {
        debug_assert!(!self.finished());
        Self::new(self.state | Self::FINISHED)
    }

    #[must_use]
    fn poison(&self) -> Self {
        Self::new(self.state | Self::POISONED)
    }

    #[must_use]
    fn unpoison(&self) -> Self {
        Self::new(self.state ^ Self::POISONED)
    }
}
