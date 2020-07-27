use std::marker::PhantomData;
use std::sync::{
    Barrier, BarrierWaitResult, Mutex, MutexGuard, Once, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

/// PhantomData implementing !Send
///
/// Structs will implement !Send automatically to own property of this type
/// without consuming any memory.
pub type PhantomMutex<T> = PhantomData<Mutex<T>>;
pub type PhantomMutexGuard<'a, T> = PhantomData<MutexGuard<'a, T>>;
pub type PhantomRwLock<T> = PhantomData<RwLock<T>>;
pub type PhantomRwLockReadGuard<'a, T> = PhantomData<RwLockReadGuard<'a, T>>;
pub type PhantomRwLockWriteGuard<'a, T> = PhantomData<RwLockWriteGuard<'a, T>>;
pub type PhantomOnce = PhantomData<Once>;
pub type PhantomBarrier = PhantomData<Barrier>;
pub type PhantomBarrierWaitResult = PhantomData<BarrierWaitResult>;
