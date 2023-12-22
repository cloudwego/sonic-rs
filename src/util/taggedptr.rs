use std::mem::transmute;

/// TaggpedPtr is a pointer of T with a tag.
#[derive(Debug)]
pub(crate) struct TaggedPtr<T> {
    // ptr is allow null ptr
    ptr: *const u8,
    _marker: std::marker::PhantomData<*const T>,
}

impl<T> TaggedPtr<T> {
    const TAG_MASK: usize = std::mem::align_of::<T>() - 1;
    const PTR_MASK: usize = !Self::TAG_MASK;

    #[inline]
    pub const fn new(ptr: *const T, tag: usize) -> Self {
        let mut slf = Self {
            ptr: ptr.cast(),
            _marker: std::marker::PhantomData,
        };
        #[allow(clippy::transmutes_expressible_as_ptr_casts)]
        let mut raw: usize = unsafe { transmute(slf.ptr) };
        raw |= tag;
        slf.ptr = raw as *const u8;
        slf
    }

    #[inline]
    pub fn set_tag(&mut self, tag: usize) {
        let mut raw = self.ptr as usize;
        raw &= Self::PTR_MASK;
        raw |= tag;
        self.ptr = raw as *const u8;
    }

    #[inline]
    pub fn tag(&self) -> usize {
        (self.ptr as usize) & Self::TAG_MASK
    }

    #[inline]
    pub fn ptr(&self) -> *const T {
        ((self.ptr as usize) & Self::PTR_MASK) as *const T
    }

    #[inline]
    pub fn set_ptr(&mut self, ptr: *const T) {
        let tag = self.tag();
        self.ptr = ((ptr as usize) | tag) as *const u8;
    }

    #[inline]
    pub fn addr(&self) -> usize {
        self.ptr as usize
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[repr(align(16))]
    struct Fox;

    #[test]
    fn test_taggedptr() {
        let f = Fox;
        let f2 = Fox;
        let mut tagged: TaggedPtr<Fox> = TaggedPtr::new(&f as *const _, 0xf);
        assert_eq!(tagged.tag(), 0xf);
        tagged.set_tag(2);
        assert_eq!(tagged.tag(), 2);
        assert_eq!(tagged.ptr(), &f as *const _);

        tagged.set_ptr(&f2 as *const _);
        assert_eq!(tagged.tag(), 2);
        assert_eq!(tagged.ptr(), &f2 as *const _);
    }
}
