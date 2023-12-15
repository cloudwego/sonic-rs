use crate::LazyValue;
use crate::RawValue;

impl<'de> From<&'de RawValue> for LazyValue<'de> {
    #[inline]
    fn from(raw: &'de RawValue) -> Self {
        Self::new(raw.as_ref().into())
    }
}
