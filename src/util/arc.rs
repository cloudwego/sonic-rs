use std::{
    fmt,
    fmt::{Display, Formatter},
    marker::PhantomData,
    ops::Deref,
    ptr::NonNull,
    sync::{
        atomic,
        atomic::Ordering::{Acquire, Relaxed, Release},
    },
};

macro_rules! field_offset {
    ($type:ty, $field:tt) => {{
        let dummy = std::mem::MaybeUninit::<$type>::uninit();
        let dummy_ptr = dummy.as_ptr();
        let member_ptr = unsafe { std::ptr::addr_of!((*dummy_ptr).$field) };
        member_ptr as usize - dummy_ptr as usize
    }};
}

pub struct Arc<T> {
    ptr: NonNull<ArcInner<T>>,
    phantom: PhantomData<ArcInner<T>>,
}

impl<T> Arc<T> {
    pub fn new(data: T) -> Arc<T> {
        let x = Box::into_raw(Box::new(ArcInner {
            refcnt: atomic::AtomicUsize::new(1),
            data,
        }));
        Self {
            ptr: unsafe { NonNull::new_unchecked(x) },
            phantom: PhantomData,
        }
    }

    #[inline]
    pub(crate) unsafe fn from_raw(ptr: *const T) -> Arc<T> {
        let offset = field_offset!(ArcInner<T>, data);
        let ptr = (ptr as *mut u8).sub(offset) as *mut ArcInner<T>;
        Self {
            ptr: NonNull::new_unchecked(ptr),
            phantom: PhantomData,
        }
    }

    #[inline]
    pub(crate) unsafe fn clone_from_raw(ptr: *const T) -> Arc<T> {
        let now = Self::from_raw(ptr);
        let ret = now.clone();
        std::mem::forget(now);
        ret
    }

    #[inline]
    pub(crate) fn inner(&self) -> &ArcInner<T> {
        unsafe { self.ptr.as_ref() }
    }

    #[inline]
    pub(crate) fn refcnt(&self) -> usize {
        self.inner().refcnt.load(Relaxed)
    }

    #[inline]
    pub(crate) fn inner_ptr(&self) -> *const ArcInner<T> {
        self.ptr.as_ptr()
    }

    #[inline]
    pub(crate) fn data_ptr(&self) -> *const T {
        &self.inner().data as *const T
    }
}

impl<T> Deref for Arc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.inner().data
    }
}

impl<T: Display> Display for Arc<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&**self, f)
    }
}

impl<T> Drop for Arc<T> {
    #[inline]
    fn drop(&mut self) {
        // Because `fetch_sub` is already atomic, we do not need to synchronize
        // with other threads unless we are going to delete the object.
        if self.inner().refcnt.fetch_sub(1, Release) != 1 {
            return;
        }

        // TODO: fix me when using ThreadSanitizer
        // ThreadSanitizer does not support memory fences. To avoid false positive
        // reports in Arc / Weak implementation use atomic loads for synchronization
        // instead.
        atomic::fence(Acquire);

        let inner = unsafe { Box::from_raw(self.ptr.as_ptr()) };
        drop(inner);
    }
}

impl<T> Clone for Arc<T> {
    #[inline]
    fn clone(&self) -> Arc<T> {
        // Using a relaxed ordering is alright here, as knowledge of the
        // original reference prevents other threads from erroneously deleting
        // the object.
        //
        self.inner().refcnt.fetch_add(1, Relaxed);

        Self {
            ptr: self.ptr,
            phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub(crate) struct ArcInner<T> {
    data: T,
    refcnt: atomic::AtomicUsize,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_arc() {
        let x = Arc::new(42);
        let y = x.clone();
        assert_eq!(*x, 42);
        assert_eq!(*y, 42);
    }
}
