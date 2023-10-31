use crate::util::private::Sealed;
use std::{marker::PhantomData, ops::Deref, ptr::NonNull};

// support borrow for owned deserizlie or skip
pub(crate) enum Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    Borrowed(&'b T),
    Copied(&'c T),
}

pub(crate) struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub(crate) fn from_index(mut i: usize, data: &[u8]) -> Self {
        // i must not exceed the length of data
        i = i.min(data.len());
        let mut position = Position { line: 1, column: 0 };
        for ch in &data[..i] {
            match *ch {
                b'\n' => {
                    position.line += 1;
                    position.column = 0;
                }
                _ => {
                    position.column += 1;
                }
            }
        }
        position
    }
}

impl<'b, 'c, T> Deref for Reference<'b, 'c, T>
where
    T: ?Sized + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match *self {
            Reference::Borrowed(b) => b,
            Reference::Copied(c) => c,
        }
    }
}

// Reader is a unified wrapper for inputs.
pub trait Reader<'de>: Sealed {
    fn remain(&self) -> isize;
    fn eat(&mut self, n: usize);
    fn backward(&mut self, n: usize);
    fn peek_n(&mut self, n: usize) -> Option<&'de [u8]>;
    fn peek(&mut self) -> Option<u8>;
    fn index(&self) -> usize;
    fn at(&self, index: usize) -> u8;
    fn set_index(&mut self, index: usize);
    fn next_n(&mut self, n: usize) -> Option<&'de [u8]>;
    fn next(&mut self) -> Option<u8> {
        self.peek().map(|a| {
            self.eat(1);
            a
        })
    }
    unsafe fn cur_ptr(&mut self) -> *mut u8;
    unsafe fn set_ptr(&mut self, cur: *mut u8);
    fn slice_unchecked(&self, start: usize, end: usize) -> &'de [u8];

    fn as_u8_slice(&self) -> &'de [u8];
}

/// JSON input source that reads from a slice of bytes.
pub struct SliceRead<'a> {
    slice: &'a [u8],
    pub(crate) index: usize,
}

impl<'a> SliceRead<'a> {
    pub fn new(slice: &'a [u8]) -> Self {
        Self { slice, index: 0 }
    }
}

impl<'a> Reader<'a> for SliceRead<'a> {
    #[inline(always)]
    fn remain(&self) -> isize {
        self.slice.len() as isize - self.index as isize
    }

    #[inline(always)]
    fn peek_n(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.index + n;
        (end <= self.slice.len()).then(|| {
            let ptr = self.slice[self.index..].as_ptr();
            unsafe { std::slice::from_raw_parts(ptr, n) }
        })
    }

    #[inline(always)]
    fn set_index(&mut self, index: usize) {
        self.index = index
    }

    #[inline(always)]
    fn peek(&mut self) -> Option<u8> {
        if self.index < self.slice.len() {
            Some(self.slice[self.index])
        } else {
            None
        }
    }

    #[inline(always)]
    fn at(&self, index: usize) -> u8 {
        self.slice[index]
    }

    #[inline(always)]
    fn next_n(&mut self, n: usize) -> Option<&'a [u8]> {
        let new_index = self.index + n;
        if new_index <= self.slice.len() {
            let ret = &self.slice[self.index..new_index];
            self.index = new_index;
            Some(ret)
        } else {
            None
        }
    }

    #[inline(always)]
    unsafe fn cur_ptr(&mut self) -> *mut u8 {
        todo!()
    }

    #[inline(always)]
    unsafe fn set_ptr(&mut self, _cur: *mut u8) {
        todo!()
    }

    #[inline(always)]
    fn index(&self) -> usize {
        self.index
    }

    #[inline(always)]
    fn eat(&mut self, n: usize) {
        self.index += n;
    }

    #[inline(always)]
    fn backward(&mut self, n: usize) {
        self.index -= n;
    }

    #[inline(always)]
    fn slice_unchecked(&self, start: usize, end: usize) -> &'a [u8] {
        &self.slice[start..end]
    }

    #[inline(always)]
    fn as_u8_slice(&self) -> &'a [u8] {
        self.slice
    }
}

pub(crate) struct UncheckedSliceRead<'a> {
    base: NonNull<u8>,
    cur: NonNull<u8>,
    len: usize,
    _life: PhantomData<&'a mut [u8]>,
}

impl<'a> UncheckedSliceRead<'a> {
    const PADDING_SIZE: usize = 64;
    pub fn new(slice: &'a mut [u8]) -> Self {
        let base = unsafe { NonNull::new_unchecked(slice.as_mut_ptr()) };
        Self {
            base,
            cur: base,
            len: slice.len() - Self::PADDING_SIZE,
            _life: PhantomData,
        }
    }
}

impl<'a> Reader<'a> for UncheckedSliceRead<'a> {
    #[inline(always)]
    fn as_u8_slice(&self) -> &'a [u8] {
        unsafe { std::slice::from_raw_parts(self.base.as_ptr(), self.len) }
    }

    #[inline(always)]
    fn remain(&self) -> isize {
        self.len as isize - self.index() as isize
    }

    #[inline(always)]
    fn peek_n(&mut self, n: usize) -> Option<&'a [u8]> {
        unsafe { Some(std::slice::from_raw_parts(self.cur.as_ptr(), n)) }
    }

    #[inline(always)]
    fn set_index(&mut self, index: usize) {
        unsafe { self.cur = NonNull::new_unchecked(self.base.as_ptr().add(index)) }
    }

    #[inline(always)]
    fn peek(&mut self) -> Option<u8> {
        unsafe { Some(*self.cur.as_ptr()) }
    }

    #[inline(always)]
    fn at(&self, index: usize) -> u8 {
        unsafe { *(self.base.as_ptr().add(index)) }
    }

    #[inline(always)]
    fn next_n(&mut self, n: usize) -> Option<&'a [u8]> {
        unsafe {
            let ptr = self.cur.as_ptr();
            self.cur = NonNull::new_unchecked(ptr.add(n));
            Some(std::slice::from_raw_parts(ptr, n))
        }
    }

    #[inline(always)]
    fn index(&self) -> usize {
        unsafe { self.cur.as_ptr().offset_from(self.base.as_ptr()) as usize }
    }

    fn eat(&mut self, n: usize) {
        unsafe {
            self.cur = NonNull::new_unchecked(self.cur.as_ptr().add(n));
        }
    }

    #[inline(always)]
    unsafe fn cur_ptr(&mut self) -> *mut u8 {
        self.cur.as_ptr()
    }

    #[inline(always)]
    unsafe fn set_ptr(&mut self, cur: *mut u8) {
        self.cur = NonNull::new_unchecked(cur);
    }

    #[inline(always)]
    fn backward(&mut self, n: usize) {
        unsafe {
            self.cur = NonNull::new_unchecked(self.cur.as_ptr().sub(n));
        }
    }

    #[inline(always)]
    fn slice_unchecked(&self, start: usize, end: usize) -> &'a [u8] {
        unsafe {
            let ptr = self.base.as_ptr().add(start);
            let n = end - start;
            std::slice::from_raw_parts(ptr, n)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn test_peek() {
        let data = b"1234567890";
        let mut reader = SliceRead::new(data);
        assert_eq!(reader.peek(), Some(b'1'));
        assert_eq!(reader.peek_n(4).unwrap(), &b"1234"[..]);
    }

    fn test_next() {
        let data = b"1234567890";
        let mut reader = SliceRead::new(data);
        assert_eq!(reader.next(), Some(b'1'));
        assert_eq!(reader.peek(), Some(b'2'));
        assert_eq!(reader.next_n(4).unwrap(), &b"2345"[..]);
        assert_eq!(reader.peek(), Some(b'6'));
    }

    fn test_index() {
        let data = b"1234567890";
        let mut reader = SliceRead::new(data);
        assert_eq!(reader.index(), 0);

        reader.next().unwrap();
        assert_eq!(reader.index(), 1);

        reader.next_n(4).unwrap();
        assert_eq!(reader.index(), 5);
    }

    #[test]
    fn test_reader() {
        test_peek();
        test_next();
        test_index();
    }
}
