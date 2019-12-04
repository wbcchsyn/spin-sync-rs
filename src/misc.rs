use std::env::Vars;
use std::marker::PhantomData;

/// Vars is an example to implement !Send.
type NotSend = Vars;

/// PhantomData implementing !Send
///
/// Structs will implement !Send automatically to own property of this type
/// without consuming any memory.
pub type PhantomNotSend = PhantomData<NotSend>;
