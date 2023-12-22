use bytes::buf::Writer;
use bytes::BytesMut;
use std::io;
use std::io::BufWriter;
use std::mem::MaybeUninit;
use std::slice::from_raw_parts_mut;

/// The trait is a extension to [`io::Write`] with a reserved capacity.
pub trait WriteExt: io::Write {
    /// Reserve with `additional` capacity and returns the remaining spare capacity of the write as a slice of
    /// `MaybeUninit<u8>`.
    ///
    /// The returned slice will be used to write new data before marking the data as initialized using the
    /// [`add_len`] method.
    ///
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]>;

    /// Flush the `additional` length to the output stream, ensuring that `additional` bytes intermediately buffered contents reach their destination.
    ///
    /// # Safety
    ///
    /// Must be used after `reserve_with`
    unsafe fn flush_len(&mut self, additional: usize);
}

impl WriteExt for Vec<u8> {
    #[inline(always)]
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        self.reserve(additional);
        unsafe {
            let ptr = self.as_mut_ptr().add(self.len()) as *mut MaybeUninit<u8>;
            Ok(from_raw_parts_mut(ptr, additional))
        }
    }

    #[inline(always)]
    unsafe fn flush_len(&mut self, additional: usize) {
        unsafe {
            let new_len = self.len() + additional;
            self.set_len(new_len);
        }
    }
}

impl WriteExt for Writer<BytesMut> {
    #[inline(always)]
    unsafe fn flush_len(&mut self, additional: usize) {
        let new_len = self.get_ref().len() + additional;
        self.get_mut().set_len(new_len);
    }

    #[inline(always)]
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        self.get_mut().reserve(additional);
        unsafe {
            let ptr = self.get_mut().as_mut_ptr().add(self.get_ref().len()) as *mut MaybeUninit<u8>;
            Ok(from_raw_parts_mut(ptr, additional))
        }
    }
}

impl<W: WriteExt + ?Sized> WriteExt for BufWriter<W> {
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        self.get_mut().reserve_with(additional)
    }

    unsafe fn flush_len(&mut self, additional: usize) {
        self.get_mut().flush_len(additional)
    }
}

impl<W: WriteExt + ?Sized> WriteExt for &mut W {
    #[inline(always)]
    unsafe fn flush_len(&mut self, additional: usize) {
        (*self).flush_len(additional)
    }

    #[inline(always)]
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        (*self).reserve_with(additional)
    }
}

impl<W: WriteExt + ?Sized> WriteExt for Box<W> {
    #[inline(always)]
    unsafe fn flush_len(&mut self, additional: usize) {
        (**self).flush_len(additional)
    }

    #[inline(always)]
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        (**self).reserve_with(additional)
    }
}

#[cfg(test)]
mod test {
    use bytes::{BufMut, BytesMut};

    use crate::writer::WriteExt;
    use std::io::Write;

    #[test]
    fn test_writer() {
        let buffer = BytesMut::new();
        let writer = &mut buffer.writer();

        let buf = writer.reserve_with(20).unwrap_or_default();
        assert_eq!(buf.len(), 20);
        assert_eq!(writer.get_ref().capacity(), 20);

        let data = b"Hello, World!";
        writer.write_all(&data[..]).unwrap();
        assert_eq!(writer.get_ref().capacity(), 20);
        assert_eq!(writer.get_ref().as_ref(), &data[..]);
    }
}
