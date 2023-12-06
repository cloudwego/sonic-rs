use bumpalo::Bump;
use parking_lot::Mutex;
use std::{alloc::Layout, ptr::NonNull};

#[derive(Debug)]
pub(crate) struct SyncBump(pub(crate) Mutex<Bump>);

#[derive(Debug)]
pub(crate) struct AllocError;

impl std::error::Error for AllocError {}

impl std::fmt::Display for AllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("alloc error")
    }
}

pub(crate) trait AllocatorTrait {
    fn try_alloc_layout(&self, layout: Layout) -> Result<NonNull<u8>, AllocError>;

    fn deallocate(&self, ptr: NonNull<u8>, layout: Layout);

    fn allocate(&self, layout: Layout) -> NonNull<u8> {
        self.try_alloc_layout(layout).expect("OOM, too big layout")
    }

    #[allow(clippy::mut_from_ref)]
    fn alloc_str(&self, s: &str) -> &mut str {
        let layout = Layout::from_size_align(s.len(), 1).unwrap();
        let ptr = self.allocate(layout);
        unsafe {
            std::ptr::copy_nonoverlapping(s.as_ptr(), ptr.as_ptr(), s.len());
        }
        unsafe {
            std::str::from_utf8_unchecked_mut(std::slice::from_raw_parts_mut(ptr.as_ptr(), s.len()))
        }
    }

    #[allow(clippy::mut_from_ref)]
    fn alloc_slice<T>(&self, len: usize) -> &mut [T] {
        let layout = Layout::array::<T>(len).expect("OOM, too big layout");
        let ptr = self.allocate(layout);
        unsafe { std::slice::from_raw_parts_mut(ptr.as_ptr() as *mut T, len) }
    }
}

impl Default for SyncBump {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncBump {
    pub fn new() -> Self {
        Self(Mutex::new(Bump::new()))
    }
}

impl AllocatorTrait for SyncBump {
    fn try_alloc_layout(&self, layout: Layout) -> Result<NonNull<u8>, AllocError> {
        self.0
            .lock()
            .try_alloc_layout(layout)
            .map_err(|_| AllocError)
    }

    fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {}
}
