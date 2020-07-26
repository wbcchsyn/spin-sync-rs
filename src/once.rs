use crate::misc::PhantomOnce;
use std::sync::atomic::{AtomicU8, Ordering};

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

    /// Performs an initialization routine once and only once. The given closure will be executed
    /// if this is the first time `call_once` has been called, and otherwise the routine will not be invoked.
    ///
    /// This method will block the calling thread if another initialization routine is currently running.
    ///
    /// When this function returns, it is guaranteed that some initialization has run and completed
    /// (it may not be the closure specified). It is also guaranteed that any memory writes performed
    /// by the executed closure can be reliably observed by other threads at this point (there is a happens-before
    /// relation between the closure and code executing after the return).
    ///
    /// If the given closure recursively invokes `call_once` on the same `Once` instance the exact behavior
    /// is not specified, allowed outcomes are a panic or a deadlock.
    ///
    /// # Examples
    ///
    /// `Once` enable to access static mut data safely.
    ///
    /// ```
    /// use spin_sync::Once;
    ///
    /// static mut CACHE: usize = 0;
    /// static INIT: Once = Once::new();
    ///
    /// fn expensive_calculation(val: usize) -> usize {
    ///     unsafe {
    ///         INIT.call_once(|| { CACHE = val; });
    ///         CACHE
    ///     }
    /// }
    ///
    /// // INIT.call_once() invokes the closure and set the CACHE.
    /// assert_eq!(1, expensive_calculation(1));
    ///
    /// // INIT.call_once() do nothing and return the CACHE.
    /// assert_eq!(1, expensive_calculation(2));
    /// ```
    ///
    /// # Panics
    ///
    /// The closure f will only be executed once if this is called concurrently among
    /// many threads. If that closure panics, however, then it will poison this `Once` instance,
    /// causing all future invocations of `call_once` to also panic.
    pub fn call_once<F: FnOnce()>(&self, f: F) {
        let (_guard, s) = self.lock();

        if s.poisoned() {
            panic!("`Once.call_once()' is called while the instance is poisoned.");
        }

        if s.finished() {
            return;
        }

        f();
        let s = s.finish();
        self.state.store(s.state, Ordering::Relaxed);
    }

    /// Performs the same function as `call_once` except ignores poisoning.
    ///
    /// Unlike `call_once`, if this `Once` has been poisoned (i.e., a previous call to `call_once`
    /// or `call_once_force` caused a panic), calling `call_once_force` will still invoke the closure
    /// f and will not result in an immediate panic. If f panics, the `Once` will remain in a poison state.
    /// If f does not panic, the `Once` will no longer be in a poison state and all future calls to
    /// `call_once` or `call_once_force` will be no-ops.
    ///
    /// The closure f is yielded a `OnceState` structure which can be used to query the poison status of the `Once`.
    ///
    /// # Examples
    ///
    /// ```
    /// use spin_sync::Once;
    /// use std::thread;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// // Poison INIT
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| panic!());
    /// });
    /// assert!(handle.join().is_err());
    ///
    /// // Poisoning propagates
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| {});
    /// });
    /// assert!(handle.join().is_err());
    ///
    /// // call_once_force will still run and reset the poisoned state
    /// INIT.call_once_force(|state| {
    ///     assert!(state.poisoned());
    /// });
    ///
    /// // once any success happens, we stop propagating the poison
    /// INIT.call_once(|| {});
    /// ```
    pub fn call_once_force<F: FnOnce(&OnceState)>(&self, f: F) {
        let (_guard, s) = self.lock();

        if s.finished() {
            return;
        }

        f(&s);
        let s = s.finish();
        let s = s.unpoison();
        self.state.store(s.state, Ordering::Relaxed);
    }

    /// Returns true if some `call_once` call has completed successfully.
    /// Specifically, `is_completed` will return false in the following situations:
    ///
    /// * `call_once` was not called at all,
    /// * `call_once` was called, but has not yet completed,
    /// * the `Once` instance is poisoned
    ///
    /// This function returning false does not mean that `Once` has not been executed.
    /// For example, it may have been executed in the time between when `is_completed`
    /// starts executing and when it returns, in which case the false return value would
    /// be stale (but still permissible).
    ///
    /// # Examples
    ///
    /// `call_once` was succeeded.
    ///
    /// ```
    /// use spin_sync::Once;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// assert_eq!(INIT.is_completed(), false);
    /// INIT.call_once(|| {
    ///     assert_eq!(INIT.is_completed(), false);
    /// });
    /// assert_eq!(INIT.is_completed(), true);
    /// ```
    ///
    /// `call_once` caused panic.
    ///
    /// ```
    /// use spin_sync::Once;
    /// use std::thread;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// assert_eq!(INIT.is_completed(), false);
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| panic!());
    /// });
    /// assert!(handle.join().is_err());
    /// assert_eq!(INIT.is_completed(), false);
    /// ```
    #[must_use]
    pub fn is_completed(&self) -> bool {
        let s = OnceState::new(self.state.load(Ordering::Relaxed));
        (!s.poisoned()) && (s.finished())
    }

    fn lock(&self) -> (OnceGuard, OnceState) {
        let mut expected = OnceState::default();
        loop {
            let desired = expected.acquire_lock();

            let current = OnceState::new(self.state.compare_and_swap(
                expected.state,
                desired.state,
                Ordering::Acquire,
            ));

            // self is locked now. Try again later.
            if current.locked() {
                expected = current.release_lock();
                std::thread::yield_now();
                continue;
            }

            // Succeed
            if current.state == expected.state {
                return (OnceGuard { once: &self }, desired);
            }

            // expected was wrong.
            expected = current;
        }
    }
}

struct OnceGuard<'a> {
    once: &'a Once,
}

impl Drop for OnceGuard<'_> {
    fn drop(&mut self) {
        let mut s = OnceState::new(self.once.state.load(Ordering::Relaxed));
        debug_assert!(s.locked());

        if std::thread::panicking() {
            s = s.poison();
        }

        s = s.release_lock();
        self.once.state.store(s.state, Ordering::Release);
    }
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
    /// Returns true if the associated `Once` was poisoned prior to the invocation of the closure
    /// passed to `call_once_force`.
    pub const fn poisoned(&self) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_once_invoke_task_only_once() {
        let mut val = 0;
        let once = Once::new();

        assert_eq!(0, val);

        once.call_once(|| val = 1);
        assert_eq!(1, val);

        once.call_once(|| val = 2);
        assert_eq!(1, val);
    }

    #[test]
    fn call_once_force_do_nothing_after_call_once_succeeded() {
        let mut val = 0;
        let once = Once::new();

        assert_eq!(0, val);

        once.call_once(|| val = 1);
        assert_eq!(1, val);

        once.call_once_force(|_| val = 2);
        assert_eq!(1, val);
    }
}
