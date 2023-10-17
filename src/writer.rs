use bytes::buf::Writer;
use bytes::BytesMut;
use std::io;
use std::slice::from_raw_parts_mut;

use std::mem::MaybeUninit;

pub trait WriterExt: io::Write {
    /// rerserve with additional space, equal as vector/bufmut reserve, but return the reserved buffer at [len: cap]
    /// # Safety
    /// must be used with add len
    unsafe fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]>;

    /// add len to the writer
    /// # Safety
    /// must be used after reserve_with
    unsafe fn add_len(&mut self, additional: usize);
}

impl WriterExt for &mut Vec<u8> {
    unsafe fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        self.reserve(additional);
        unsafe {
            let ptr = self.as_mut_ptr().add(self.len()) as *mut MaybeUninit<u8>;
            Ok(from_raw_parts_mut(ptr, additional))
        }
    }

    unsafe fn add_len(&mut self, additional: usize) {
        unsafe {
            let new_len = self.len() + additional;
            self.set_len(new_len);
        }
    }
}

impl WriterExt for Writer<BytesMut> {
    unsafe fn add_len(&mut self, additional: usize) {
        let new_len = self.get_ref().len() + additional;
        self.get_mut().set_len(new_len);
    }

    unsafe fn reserve_with(&mut self, additional: usize) -> io::Result<&mut [MaybeUninit<u8>]> {
        self.get_mut().reserve(additional);
        unsafe {
            let ptr = self.get_mut().as_mut_ptr().add(self.get_ref().len()) as *mut MaybeUninit<u8>;
            Ok(from_raw_parts_mut(ptr, additional))
        }
    }
}

#[cfg(test)]
mod test {
    use crate::writer::WriterExt;
    use std::io::Write;

    #[test]
    fn test_writer() {
        let mut buffer: Vec<u8> = Vec::new();
        let mut writer = &mut buffer;

        let buf = unsafe { writer.reserve_with(20) }.unwrap_or_default();
        assert_eq!(buf.len(), 20);
        assert_eq!(writer.capacity(), 20);

        let data = b"Hello, World!";
        writer.write_all(&data[..]).unwrap();
        assert_eq!(writer.capacity(), 20);
        assert_eq!(writer, &data[..]);
    }
}
