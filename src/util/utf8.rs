use crate::error::{Error, ErrorCode, Result};

// simduft8 will cause `out-of-bounds pointer arithmetic` when using Miri tests
#[cfg(not(miri))]
#[inline]
pub(crate) fn from_utf8(data: &[u8]) -> Result<&str> {
    simdutf8::basic::from_utf8(data).or_else(|_| from_utf8_compat(data))
}

#[cfg(miri)]
pub(crate) fn from_utf8(data: &[u8]) -> Result<&str> {
    std::str::from_utf8(data)
        .map_err(|e| Error::syntax(ErrorCode::InvalidUTF8, data, e.valid_up_to()))
}

#[cfg(not(miri))]
#[cold]
fn from_utf8_compat(data: &[u8]) -> Result<&str> {
    // compat::from_utf8 is slower than basic::from_utf8
    simdutf8::compat::from_utf8(data)
        .map_err(|e| Error::syntax(ErrorCode::InvalidUTF8, data, e.valid_up_to()))
}
