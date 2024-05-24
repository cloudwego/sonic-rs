use super::Value;
use crate::{LazyValue, Number};

impl TryFrom<f32> for Value {
    type Error = crate::Error;

    /// Try convert a f32 to `Value`. If the float is NaN or infinity, return a error.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{JsonValueTrait, Value};
    ///
    /// let f1: f32 = 2.333;
    /// let x1: Value = f1.try_into().unwrap();
    /// assert_eq!(x1, f1);
    ///
    /// let x2: Value = f32::INFINITY.try_into().unwrap_or_default();
    /// let x3: Value = f32::NAN.try_into().unwrap_or_default();
    ///
    /// assert!(x2.is_null() && x3.is_null());
    /// ```
    #[inline]
    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Number::try_from(value).map(Into::into)
    }
}

impl TryFrom<f64> for Value {
    /// Try convert a f64 to `Value`. If the float is NaN or infinity, return a error.
    ///
    /// # Examples
    ///
    /// ```
    /// use sonic_rs::{JsonValueTrait, Value};
    ///
    /// let f1: f64 = 2.333;
    /// let x1: Value = f1.try_into().unwrap();
    /// assert_eq!(x1, 2.333);
    ///
    /// let x2: Value = f64::INFINITY.try_into().unwrap_or_default();
    /// let x3: Value = f64::NAN.try_into().unwrap_or_default();
    ///
    /// assert!(x2.is_null() && x3.is_null());
    /// ```
    type Error = crate::Error;
    #[inline]
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Number::try_from(value).map(Into::into)
    }
}

/// Try parse a `LazyValue` into a `Value`.  `LazyValue` is always a valid JSON, at least it is
/// followed the JSON syntax.
///
/// However, in some cases, the parse will failed and return errors, such as the float number in
/// JSON is inifity.
impl<'de> TryFrom<LazyValue<'de>> for Value {
    type Error = crate::Error;
    fn try_from(value: LazyValue<'de>) -> Result<Self, Self::Error> {
        crate::from_str(value.as_raw_str())
    }
}
