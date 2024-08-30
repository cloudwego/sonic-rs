use std::{
    cell::UnsafeCell,
    fmt::{Debug, Formatter},
    mem::ManuallyDrop,
    sync::atomic::AtomicBool,
};

use super::allocator::SyncBump;
use crate::util::{arc::Arc, taggedptr::TaggedPtr};

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

//////////////////////////////////////////////////////////////////////////////

// We use a thread local to make the allocator can be used in whole `From` trait.
thread_local! {
    static SHARED: UnsafeCell<*const Shared> = const { UnsafeCell::new(std::ptr::null()) };
}

pub(crate) fn get_shared() -> *const Shared {
    SHARED.with(|shared| unsafe { *shared.get() })
}

pub(crate) fn get_shared_or_new() -> (&'static Shared, bool) {
    let shared = SHARED.with(|shared| unsafe { *shared.get() });
    if shared.is_null() {
        let arc = ManuallyDrop::new(Arc::new(Shared::new()));
        (unsafe { &*arc.data_ptr() }, true)
    } else {
        (unsafe { &*shared }, false)
    }
}

pub(crate) fn set_shared(new_shared: *const Shared) {
    SHARED.with(|shared| unsafe { *((*shared).get()) = new_shared });
}

pub(crate) struct SharedCtxGuard {
    old: *const Shared,
}

impl SharedCtxGuard {
    /// assign `new_shared` into SharedCtx
    pub(crate) fn assign(new_shared: *const Shared) -> Self {
        let old = get_shared();
        set_shared(new_shared);
        Self { old }
    }
}

impl Drop for SharedCtxGuard {
    fn drop(&mut self) {
        set_shared(self.old);
    }
}
