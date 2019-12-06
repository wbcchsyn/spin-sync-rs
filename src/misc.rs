use std::marker::PhantomData;
use std::sync::MutexGuard;

type NotSend<'a, T> = MutexGuard<'a, T>;

/// PhantomData implementing !Send
///
/// Structs will implement !Send automatically to own property of this type
/// without consuming any memory.
pub type PhantomNotSend<'a, T> = PhantomData<NotSend<'a, T>>;
