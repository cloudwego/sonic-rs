use super::alloctor::SyncBump;
use crate::util::{arc::Arc, taggedptr::TaggedPtr};
use std::{
    fmt::{Debug, Formatter},
    mem::ManuallyDrop,
    sync::atomic::AtomicBool,
};

// Represent a shared allocator.
#[derive(Debug)]
#[repr(align(16))]
#[doc(hidden)]
pub struct Shared {
    pub(crate) alloc: SyncBump,
    // Whether there are multiple allocator in the `Value` tree.
    // the flag is conservative to make sure no memory leaks, more details see `Drop` comments.
    pub(crate) combined: AtomicBool,
}

impl Debug for Arc<Shared> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("refcnt", &self.refcnt())
            .field("combined", &self.combined)
            .field("allocator address", &self.alloc.0.data_ptr())
            .finish()
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

impl Shared {
    pub(crate) fn new() -> Self {
        Self {
            alloc: SyncBump::new(),
            combined: AtomicBool::new(false),
        }
    }

    #[doc(hidden)]
    pub fn new_ptr() -> *const Shared {
        ManuallyDrop::new(Arc::new(Shared::new())).data_ptr()
    }

    /// there are no way to convert the `combined` from true to false
    /// so we can use relaxed ordering here.
    /// And The origin refcnt of Shared prevents other threads from erroneously deleting
    // the shared allocator.
    pub(crate) fn is_combined(&self) -> bool {
        self.combined.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The origin refcnt of Shared prevents other threads from erroneously deleting
    // the shared allocator.
    pub(crate) fn set_combined(&self) {
        self.combined
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// TaggedPtr is not arc
impl Clone for TaggedPtr<Shared> {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for TaggedPtr<Shared> {}
