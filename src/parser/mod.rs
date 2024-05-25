use std::str::from_utf8_unchecked;

mod inner;

cfg_if::cfg_if! {
    if #[cfg(feature = "runtime-detection")] {
        mod runtime;
        pub(crate) use self::runtime::Parser;
    } else {
        use crate::util::simd::{i8x32, u8x32, u8x64};
        pub(crate) type Parser<R> = self::inner::Parser<R, i8x32, u8x32, u8x64>;
    }
}

pub(crate) use self::inner::ParseStatus;

pub(crate) const DEFAULT_KEY_BUF_CAPACITY: usize = 128;
pub(crate) fn as_str(data: &[u8]) -> &str {
    unsafe { from_utf8_unchecked(data) }
}
