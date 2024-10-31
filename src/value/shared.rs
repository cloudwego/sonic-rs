use std::fmt::Debug;

use bumpalo::Bump;

// Represent a shared allocator.
#[derive(Debug, Default)]
#[repr(C)]
#[doc(hidden)]
pub struct Shared {
    pub(crate) json: Vec<u8>,
    pub(crate) alloc: Bump,
}
