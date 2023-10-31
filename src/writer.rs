use bytes::buf::Writer;
use bytes::BytesMut;
use std::io;
use std::slice::from_raw_parts_mut;

use std::mem::MaybeUninit;

/// WriterExt is a extension to write with reserved space. It is designed for
/// SIMD serializing without bound-checking.
pub trait WriterExt: io::Write {
    /// rerserve with additional space, equal as vector/bufmut reserve, but return the reserved buffer at [len: cap]
    /// # Safety
    /// must be used with `add_len`
    unsafe fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]>;

    /// add len to the writer
    /// # Safety
    /// must be used after `reserve_with`
    unsafe fn add_len(&mut self, additional: usize);
}

impl WriterExt for Vec<u8> {
    #[inline(always)]
    unsafe fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        self.reserve(additional);
        unsafe {
            let ptr = self.as_mut_ptr().add(self.len()) as *mut MaybeUninit<u8>;
            Ok(from_raw_parts_mut(ptr, additional))
        }
    }

    #[inline(always)]
    unsafe fn add_len(&mut self, additional: usize) {
        unsafe {
            let new_len = self.len() + additional;
            self.set_len(new_len);
        }
    }
}

impl WriterExt for Writer<BytesMut> {
    #[inline(always)]
    unsafe fn add_len(&mut self, additional: usize) {
        let new_len = self.get_ref().len() + additional;
        self.get_mut().set_len(new_len);
    }

    #[inline(always)]
    unsafe fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        self.get_mut().reserve(additional);
        unsafe {
            let ptr = self.get_mut().as_mut_ptr().add(self.get_ref().len()) as *mut MaybeUninit<u8>;
            Ok(from_raw_parts_mut(ptr, additional))
        }
    }
}

impl<W: WriterExt + ?Sized> WriterExt for &mut W {
    #[inline(always)]
    unsafe fn add_len(&mut self, additional: usize) {
        (*self).add_len(additional)
    }

    #[inline(always)]
    unsafe fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        (*self).reserve_with(additional)
    }
}

impl<W: WriterExt + ?Sized> WriterExt for Box<W> {
    #[inline(always)]
    unsafe fn add_len(&mut self, additional: usize) {
        (**self).add_len(additional)
    }

    #[inline(always)]
    unsafe fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        (**self).reserve_with(additional)
    }
}

#[cfg(test)]
mod test {
    use bytes::{BufMut, BytesMut};

    use crate::writer::WriterExt;
    use std::io::Write;

    #[test]
    fn test_writer() {
        let buffer = BytesMut::new();
        let writer = &mut buffer.writer();

        let buf = unsafe { writer.reserve_with(20) }.unwrap_or_default();
        assert_eq!(buf.len(), 20);
        assert_eq!(writer.get_ref().capacity(), 20);

        let data = b"Hello, World!";
        writer.write_all(&data[..]).unwrap();
        assert_eq!(writer.get_ref().capacity(), 20);
        assert_eq!(writer.get_ref().as_ref(), &data[..]);
    }
}
