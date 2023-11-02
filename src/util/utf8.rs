use crate::error::{Error, ErrorCode, Result};

#[inline(always)]
pub(crate) fn from_utf8(data: &[u8]) -> Result<&str> {
    match simdutf8::basic::from_utf8(data) {
        Ok(ret) => Ok(ret),
        Err(_) => {
            // slow path, get the correct position of the first invalid utf-8 character
            from_utf8_compat(data)
        }
    }
}

#[cold]
fn from_utf8_compat(data: &[u8]) -> Result<&str> {
    // compat::from_utf8 is slower than basic::from_utf8
    match simdutf8::compat::from_utf8(data) {
        Ok(ret) => Ok(ret),
        Err(err) => Err(Error::syntax(
            ErrorCode::InvalidUTF8,
            data,
            err.valid_up_to(),
        )),
    }
}
