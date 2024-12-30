//! Extend trait from io::Write for JSON serializing.

use std::{io, io::BufWriter as IoBufWriter, mem::MaybeUninit, slice::from_raw_parts_mut};

use bytes::{buf::Writer, BytesMut};

/// The trait is a extension to [`io::Write`] with a reserved capacity.
pub trait WriteExt: io::Write {
    /// Reserve with `additional` capacity and returns the remaining spare capacity of the write as
    /// a slice of `MaybeUninit<u8>`.
    ///
    /// The returned slice will be used to write new data before marking the data as initialized
    /// using the [`WriteExt::flush_len`] method.
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]>;

    /// Flush the `additional` length to the output stream, ensuring that `additional` bytes
    /// intermediately buffered contents reach their destination.
    ///
    /// # Safety
    ///
    /// Must be used after `reserve_with`
    unsafe fn flush_len(&mut self, additional: usize) -> io::Result<()>;
}

/// Wrapper around generic I/O streams implementing [`WriteExt`]
///
/// It internally maintains a buffer for fast operations which it then flushes
/// to the underlying I/O stream when requested.
pub struct BufferedWriter<W> {
    inner: W,
    buffer: Vec<u8>,
}

impl<W> BufferedWriter<W> {
    /// Construct a new buffered writer
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
        }
    }
}

impl<W> io::Write for BufferedWriter<W>
where
    W: io::Write,
{
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W> WriteExt for BufferedWriter<W>
where
    W: io::Write,
{
    #[inline(always)]
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        self.buffer.reserve_with(additional)
    }

    #[inline(always)]
    unsafe fn flush_len(&mut self, additional: usize) -> io::Result<()> {
        self.buffer.flush_len(additional)?;
        self.inner.write_all(&self.buffer)?;
        self.buffer.clear();

        Ok(())
    }
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
    unsafe fn flush_len(&mut self, additional: usize) -> io::Result<()> {
        unsafe {
            let new_len = self.len() + additional;
            self.set_len(new_len);
        }

        Ok(())
    }
}

impl WriteExt for Writer<BytesMut> {
    #[inline(always)]
    unsafe fn flush_len(&mut self, additional: usize) -> io::Result<()> {
        let new_len = self.get_ref().len() + additional;
        self.get_mut().set_len(new_len);
        Ok(())
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

impl WriteExt for Writer<&mut BytesMut> {
    #[inline(always)]
    unsafe fn flush_len(&mut self, additional: usize) -> io::Result<()> {
        let new_len = self.get_ref().len() + additional;
        self.get_mut().set_len(new_len);
        Ok(())
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

impl<W: WriteExt + ?Sized> WriteExt for IoBufWriter<W> {
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        self.get_mut().reserve_with(additional)
    }

    unsafe fn flush_len(&mut self, additional: usize) -> io::Result<()> {
        self.get_mut().flush_len(additional)
    }
}

impl<W: WriteExt + ?Sized> WriteExt for &mut W {
    #[inline(always)]
    unsafe fn flush_len(&mut self, additional: usize) -> io::Result<()> {
        (*self).flush_len(additional)
    }

    #[inline(always)]
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        (*self).reserve_with(additional)
    }
}

impl<W: WriteExt + ?Sized> WriteExt for Box<W> {
    #[inline(always)]
    unsafe fn flush_len(&mut self, additional: usize) -> io::Result<()> {
        (**self).flush_len(additional)
    }

    #[inline(always)]
    fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        (**self).reserve_with(additional)
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use bytes::{BufMut, BytesMut};

    use crate::writer::WriteExt;

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
