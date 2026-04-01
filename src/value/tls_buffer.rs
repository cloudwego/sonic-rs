use std::{
    mem::{size_of, ManuallyDrop},
    ptr::NonNull,
};

use super::node::Value;

thread_local! {
   static NODE_BUF: std::cell::RefCell<Vec<ManuallyDrop<Value>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Thread-local buffer ownership. Handles TLS borrow or heap allocation.
struct TlsBuf {
    buf: NonNull<Vec<ManuallyDrop<Value>>>,
    need_drop: bool,
}

impl TlsBuf {
    const MAX_TLS_SIZE: usize = (3 << 20) / size_of::<Value>(); // 3 Mb

    #[inline]
    fn with_capacity(n: usize) -> Self {
        if n >= Self::MAX_TLS_SIZE {
            let vec = Box::leak(Box::new(Vec::with_capacity(n)));
            Self {
                buf: NonNull::from(vec),
                need_drop: true,
            }
        } else {
            let vec = NODE_BUF.with(|buf| {
                let mut nodes = buf.borrow_mut();
                nodes.clear();
                nodes.reserve(n);
                (&mut *nodes) as *mut Vec<ManuallyDrop<Value>>
            });

            Self {
                buf: NonNull::new(vec).expect("thread-local buffer pointer is non-null"),
                need_drop: false,
            }
        }
    }

    #[inline]
    fn as_vec_mut(&mut self) -> &mut Vec<ManuallyDrop<Value>> {
        unsafe { self.buf.as_mut() }
    }

    #[inline]
    #[allow(dead_code)] // Used by NodeBuf::len() under cfg(miri)
    fn len(&self) -> usize {
        unsafe { (*self.buf.as_ptr()).len() }
    }
}

impl Drop for TlsBuf {
    fn drop(&mut self) {
        if self.need_drop {
            let boxed: Box<Vec<ManuallyDrop<Value>>> = unsafe { Box::from_raw(self.buf.as_ptr()) };
            drop(boxed);
        }
    }
}

/// Node staging buffer for DOM construction.
///
/// Owns a `TlsBuf` and provides pointer-based or Vec-based access depending
/// on the build mode:
/// - **Production** (`cfg(not(miri))`): cached raw pointers avoid store-to-load forwarding stalls
///   on `Vec`'s `len` field.
/// - **Miri** (`cfg(miri)`): all access goes through `&mut Vec` to preserve Stacked Borrows
///   provenance for pointers embedded inside `Value` nodes.
pub(crate) struct NodeBuf {
    #[allow(dead_code)]
    // Owns Vec memory + accessed under cfg(miri); raw pointers alias it otherwise
    buf: TlsBuf,
    #[cfg(not(miri))]
    base: *mut ManuallyDrop<Value>,
    #[cfg(not(miri))]
    end: *mut ManuallyDrop<Value>,
    #[cfg(not(miri))]
    cap_end: *mut ManuallyDrop<Value>,
}

impl NodeBuf {
    pub fn with_capacity(n: usize) -> Self {
        let mut buf = TlsBuf::with_capacity(n);
        #[cfg(not(miri))]
        {
            let vec = buf.as_vec_mut();
            let base = vec.as_mut_ptr();
            let cap = vec.capacity();
            NodeBuf {
                buf,
                base,
                end: base,
                cap_end: unsafe { base.add(cap) },
            }
        }
        #[cfg(miri)]
        {
            NodeBuf { buf }
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        #[cfg(miri)]
        {
            self.buf.len()
        }
        #[cfg(not(miri))]
        unsafe {
            self.end.offset_from(self.base) as usize
        }
    }

    #[inline(always)]
    pub fn node_ref(&mut self, idx: usize) -> &ManuallyDrop<Value> {
        #[cfg(miri)]
        {
            &self.buf.as_vec_mut()[idx]
        }
        #[cfg(not(miri))]
        unsafe {
            &*self.base.add(idx)
        }
    }

    #[inline(always)]
    pub fn node_mut(&mut self, idx: usize) -> &mut ManuallyDrop<Value> {
        #[cfg(miri)]
        {
            &mut self.buf.as_vec_mut()[idx]
        }
        #[cfg(not(miri))]
        unsafe {
            &mut *self.base.add(idx)
        }
    }

    /// Push a value. Returns false if at capacity.
    #[inline(always)]
    pub fn push(&mut self, val: ManuallyDrop<Value>) -> bool {
        #[cfg(miri)]
        {
            let vec = self.buf.as_vec_mut();
            if vec.len() == vec.capacity() {
                return false;
            }
            vec.push(val);
            true
        }
        #[cfg(not(miri))]
        {
            if self.end == self.cap_end {
                false
            } else {
                unsafe {
                    self.end.write(val);
                    self.end = self.end.add(1);
                }
                true
            }
        }
    }

    /// Copy `count` nodes starting at index `from` into `dst`.
    #[inline(always)]
    pub unsafe fn copy_to(&mut self, from: usize, dst: *mut ManuallyDrop<Value>, count: usize) {
        #[cfg(miri)]
        {
            let src = self.buf.as_vec_mut()[from..from + count].as_ptr();
            std::ptr::copy_nonoverlapping(src, dst, count);
        }
        #[cfg(not(miri))]
        {
            let src = self.base.add(from);
            #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
            super::node::inline_copy_values(src, dst, count);
            #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
            std::ptr::copy_nonoverlapping(src, dst, count);
        }
    }

    /// Truncate the buffer to `new_len`.
    #[inline(always)]
    pub unsafe fn truncate(&mut self, new_len: usize) {
        #[cfg(miri)]
        {
            self.buf.as_vec_mut().set_len(new_len);
        }
        #[cfg(not(miri))]
        {
            self.end = self.base.add(new_len);
        }
    }
}
