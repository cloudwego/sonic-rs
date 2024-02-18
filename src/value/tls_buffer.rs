use std::{
    mem::{size_of, ManuallyDrop},
    ptr::NonNull,
};

use super::node::Value;

// use const make thread local access faster

thread_local! {
   static NODE_BUF: std::cell::RefCell<Vec<ManuallyDrop<Value>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A thread-local buffer for temporary nodes. Avoid allocating temporary memory multiple times.
pub struct TlsBuf {
    buf: NonNull<Vec<ManuallyDrop<Value>>>,
    need_drop: bool,
}

impl TlsBuf {
    const MAX_TLS_SIZE: usize = (3 << 20) / size_of::<Value>(); // 3 Mb

    #[inline]
    pub fn with_capacity(n: usize) -> Self {
        if n >= Self::MAX_TLS_SIZE {
            let vec = Box::into_raw(Box::new(Vec::with_capacity(n)));
            Self {
                buf: unsafe { NonNull::new_unchecked(vec) },
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
                buf: unsafe { NonNull::new_unchecked(vec) },
                need_drop: false,
            }
        }
    }

    #[inline]
    pub fn as_vec_mut(&mut self) -> &mut Vec<ManuallyDrop<Value>> {
        unsafe { self.buf.as_mut() }
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
