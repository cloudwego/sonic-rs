use bytes::Bytes;
use faststr::FastStr;

use crate::{parser::as_str, util::private::Sealed};

/// JsonSlice is a wrapper for different json input.
#[doc(hidden)]
#[non_exhaustive]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum JsonSlice<'de> {
    Raw(&'de [u8]),
    FastStr(FastStr), // note: FastStr maybe inlined and in the stack.
}

impl<'de> JsonSlice<'de> {
    #[inline(always)]
    pub(crate) unsafe fn as_faststr(&self) -> FastStr {
        match self {
            JsonSlice::Raw(sub) => FastStr::new(as_str(sub)),
            JsonSlice::FastStr(f) => f.clone(),
        }
    }
}

impl Default for JsonSlice<'_> {
    fn default() -> Self {
        JsonSlice::Raw(&b"null"[..])
    }
}

impl<'de> From<FastStr> for JsonSlice<'de> {
    fn from(value: FastStr) -> Self {
        JsonSlice::FastStr(value)
    }
}

impl<'de> From<Bytes> for JsonSlice<'de> {
    fn from(value: Bytes) -> Self {
        JsonSlice::FastStr(unsafe { FastStr::from_bytes_unchecked(value) })
    }
}

impl<'de> From<&'de [u8]> for JsonSlice<'de> {
    fn from(value: &'de [u8]) -> Self {
        JsonSlice::Raw(value)
    }
}

impl<'de> From<&'de str> for JsonSlice<'de> {
    fn from(value: &'de str) -> Self {
        JsonSlice::Raw(value.as_bytes())
    }
}

impl<'de> From<&'de String> for JsonSlice<'de> {
    fn from(value: &'de String) -> Self {
        JsonSlice::Raw(value.as_bytes())
    }
}

impl From<String> for JsonSlice<'_> {
    fn from(value: String) -> Self {
        JsonSlice::FastStr(FastStr::new(value))
    }
}

impl<'de> AsRef<[u8]> for JsonSlice<'de> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Raw(r) => r,
            Self::FastStr(s) => s.as_bytes(),
        }
    }
}

/// A trait for string/bytes-like types that can be parsed into JSON.
pub trait JsonInput<'de>: Sealed {
    fn need_utf8_valid(&self) -> bool;
    fn to_json_slice(&self) -> JsonSlice<'de>;
    #[allow(clippy::wrong_self_convention)]
    fn from_subset(&self, sub: &'de [u8]) -> JsonSlice<'de>;
    fn to_u8_slice(&self) -> &'de [u8];
}

impl<'de> JsonInput<'de> for &'de [u8] {
    fn need_utf8_valid(&self) -> bool {
        true
    }

    fn to_json_slice(&self) -> JsonSlice<'de> {
        JsonSlice::Raw(self)
    }

    fn from_subset(&self, sub: &'de [u8]) -> JsonSlice<'de> {
        sub.into()
    }

    fn to_u8_slice(&self) -> &'de [u8] {
        self
    }
}

impl<'de> JsonInput<'de> for &'de str {
    fn need_utf8_valid(&self) -> bool {
        false
    }
    fn to_json_slice(&self) -> JsonSlice<'de> {
        JsonSlice::Raw((*self).as_bytes())
    }

    fn from_subset(&self, sub: &'de [u8]) -> JsonSlice<'de> {
        sub.into()
    }

    fn to_u8_slice(&self) -> &'de [u8] {
        (*self).as_bytes()
    }
}

impl<'de> JsonInput<'de> for &'de Bytes {
    fn need_utf8_valid(&self) -> bool {
        true
    }

    fn to_json_slice(&self) -> JsonSlice<'de> {
        let bytes = self.as_ref();
        let newed = self.slice_ref(bytes);
        JsonSlice::FastStr(unsafe { FastStr::from_bytes_unchecked(newed) })
    }

    fn from_subset(&self, sub: &'de [u8]) -> JsonSlice<'de> {
        self.slice_ref(sub).into()
    }

    fn to_u8_slice(&self) -> &'de [u8] {
        (*self).as_ref()
    }
}

impl<'de> JsonInput<'de> for &'de FastStr {
    fn need_utf8_valid(&self) -> bool {
        false
    }

    fn to_json_slice(&self) -> JsonSlice<'de> {
        JsonSlice::FastStr((**self).clone())
    }

    fn from_subset(&self, sub: &'de [u8]) -> JsonSlice<'de> {
        self.slice_ref(as_str(sub)).into()
    }

    fn to_u8_slice(&self) -> &'de [u8] {
        (*self).as_ref()
    }
}

impl<'de> JsonInput<'de> for &'de String {
    fn need_utf8_valid(&self) -> bool {
        false
    }

    fn to_json_slice(&self) -> JsonSlice<'de> {
        JsonSlice::Raw(self.as_bytes())
    }

    fn from_subset(&self, sub: &'de [u8]) -> JsonSlice<'de> {
        sub.into()
    }

    fn to_u8_slice(&self) -> &'de [u8] {
        (*self).as_bytes()
    }
}
