use std::fmt::Debug;

use bumpalo::Bump;

// Represent a shared allocator.
#[derive(Debug, Default)]
#[repr(C, align(8))]
#[doc(hidden)]
pub struct Shared {
    json: Vec<u8>,
    alloc: Bump,
}

impl Shared {
    pub fn get_alloc(&mut self) -> &mut Bump {
        &mut self.alloc
    }

    pub fn set_json(&mut self, json: Vec<u8>) {
        self.json = json;
    }
}

// #safety
// we not export the immutable bump allocator, so `Sync`` is always safe here
unsafe impl Sync for Shared {}
