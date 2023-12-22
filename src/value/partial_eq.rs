use faststr::FastStr;

use super::object::Pair;
use crate::{
    value::{
        node::Value,
        value_trait::{JsonContainerTrait, JsonValueTrait},
    },
    JsonType,
};
impl Eq for Value {}

impl PartialEq for Value {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }

        if self.get_type() != other.get_type() {
            return false;
        }

        match self.get_type() {
            JsonType::Boolean => self.as_bool() == other.as_bool(),
            JsonType::Null => other.is_null(),
            JsonType::Number => self.as_f64() == other.as_f64(),
            JsonType::String => self.as_str() == other.as_str(),
            JsonType::Array => {
                let len = self.len();
                if len != other.len() {
                    return false;
                }
                let ours = self.children::<Value>();
                let theirs = other.children::<Value>();
                ours.iter().zip(theirs).all(|(a, b)| (*a) == b)
            }
            JsonType::Object => {
                let len = self.len();
                if len != other.len() {
                    return false;
                }
                if len == 0 {
                    return true;
                }

                for (k, v) in self.iter::<Pair>() {
                    let key = k.as_str().unwrap();
                    let matched = other.get(key).map(|v1| v == v1).unwrap_or(false);
                    if !matched {
                        return false;
                    }
                }
                true
            }
        }
    }
}

macro_rules! impl_str_eq {
    ($($eq:ident [$($ty:ty)*])*) => {
        $($(
            impl PartialEq<$ty> for Value {
                #[inline]
                fn eq(&self, other: &$ty) -> bool {
                    let s: &str = other.as_ref();
                    $eq(self, s)
                }
            }

            impl PartialEq<Value> for $ty {
                #[inline]
                fn eq(&self, other: &Value) -> bool {
                    let s: &str = self.as_ref();
                    $eq(other, s)
                }
            }

            impl PartialEq<$ty> for &Value {
                #[inline]
                fn eq(&self, other: &$ty) -> bool {
                    let s: &str = other.as_ref();
                    $eq(*self, s)
                }
            }

            impl PartialEq<$ty> for &mut Value {
                #[inline]
                fn eq(&self, other: &$ty) -> bool {
                    let s: &str = other.as_ref();
                    $eq(*self, s)
                }
            }
        )*)*
    }
}

impl_str_eq! {
    eq_str[str String FastStr]
}

impl PartialEq<&str> for Value {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        eq_str(self, other)
    }
}

impl PartialEq<Value> for &str {
    #[inline]
    fn eq(&self, other: &Value) -> bool {
        eq_str(other, self)
    }
}

///////////////////////////////////////////////////////////////////
// Copied from serde_json

#[inline]
fn eq_i64(value: &Value, other: i64) -> bool {
    value.as_i64().map_or(false, |i| i == other)
}

#[inline]
fn eq_u64(value: &Value, other: u64) -> bool {
    value.as_u64().map_or(false, |i| i == other)
}

#[inline]
fn eq_f64(value: &Value, other: f64) -> bool {
    value.as_f64().map_or(false, |i| i == other)
}

#[inline]
fn eq_bool(value: &Value, other: bool) -> bool {
    value.as_bool().map_or(false, |i| i == other)
}

#[inline]
fn eq_str(value: &Value, other: &str) -> bool {
    value.as_str().map_or(false, |i| i == other)
}

macro_rules! impl_numeric_eq {
    ($($eq:ident [$($ty:ty)*])*) => {
        $($(
            impl PartialEq<$ty> for Value {
                #[inline]
                fn eq(&self, other: &$ty) -> bool {
                    $eq(self, *other as _)
                }
            }

            impl PartialEq<Value> for $ty {
                #[inline]
                fn eq(&self, other: &Value) -> bool {
                    $eq(other, *self as _)
                }
            }

            impl PartialEq<$ty> for &Value {
                #[inline]
                fn eq(&self, other: &$ty) -> bool {
                    $eq(*self, *other as _)
                }
            }

            impl PartialEq<$ty> for &mut Value {
                #[inline]
                fn eq(&self, other: &$ty) -> bool {
                    $eq(*self, *other as _)
                }
            }
        )*)*
    }
}

impl_numeric_eq! {
    eq_i64[i8 i16 i32 i64 isize]
    eq_u64[u8 u16 u32 u64 usize]
    eq_f64[f32 f64]
    eq_bool[bool]
}

//////////////////////////////////////////////////////////////////////////////

macro_rules! impl_slice_eq {
    ([$($vars:tt)*], $rhs:ty $(where $ty:ty: $bound:ident)?) => {
        impl<U, $($vars)*> PartialEq<$rhs> for Array
        where
            Value: PartialEq<U>,
            $($ty: $bound)?
        {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                let len = self.len();
                if len != other.len() {
                    return false;
                }
                let slf = self.as_ref();
                let other: &[U] = other.as_ref();
                slf.iter().zip(other).all(|(a, b)| *a == *b )
            }
        }

        impl<U, $($vars)*> PartialEq<$rhs> for Value
        where
            Value: PartialEq<U>,
            $($ty: $bound)?
        {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                self.as_array().map(|arr| arr == other).unwrap_or(false)
            }
        }


        impl<U, $($vars)*> PartialEq<Array> for $rhs
        where
            Value: PartialEq<U>,
            $($ty: $bound)?
        {
            #[inline]
            fn eq(&self, other: &Array) -> bool {
                other == self
            }
        }

        impl<U, $($vars)*> PartialEq<Value> for $rhs
        where
            Value: PartialEq<U>,
            $($ty: $bound)?
        {
            #[inline]
            fn eq(&self, other: &Value) -> bool {
                other == self
            }
        }
    }
}

impl_slice_eq!([], &[U]);
impl_slice_eq!([], &mut [U]);
impl_slice_eq!([], [U]);
impl_slice_eq!([const N: usize], &[U; N]);
impl_slice_eq!([const N: usize], [U; N]);
impl_slice_eq!([], Vec<U>);

//////////////////////////////////////////////////////////////////////////////

use super::{array::Array, object::Object};

// TODO: compare value with object/array.

macro_rules! impl_container_eq {
    ($($ty:ty)*) => {
        $(
            impl PartialEq<$ty> for Value {
                #[inline]
                fn eq(&self, other: &$ty) -> bool {
                    self == &other.0
                }
            }

            impl PartialEq<Value> for $ty {
                #[inline]
                fn eq(&self, other: &Value) -> bool {
                    other == &self.0
                }
            }

            impl  PartialEq<$ty> for &Value {
                #[inline]
                fn eq(&self, other: &$ty) -> bool {
                    *self == &other.0
                }
            }

            impl  PartialEq<$ty> for &mut Value {
                #[inline]
                fn eq(&self, other: &$ty) -> bool {
                    *self == &other.0
                }
            }

            impl PartialEq<Value> for &$ty {
                #[inline]
                fn eq(&self, other: &Value) -> bool {
                    other == &self.0
                }
            }

            impl PartialEq<Value> for &mut $ty {
                #[inline]
                fn eq(&self, other: &Value) -> bool {
                    other == &self.0
                }
            }

        )*
    }
}

impl_container_eq!(Array Object);

#[cfg(test)]
mod test {
    use faststr::FastStr;

    use crate::{array, json};
    #[test]
    fn test_slice_eq() {
        assert_eq!(json!([1, 2, 3]), &[1, 2, 3]);
        assert_eq!(array![1, 2, 3], &[1, 2, 3]);
        assert_eq!(json!([1, 2, 3]), array![1, 2, 3].as_slice());

        assert_eq!(json!([1, 2, 3]), vec![1, 2, 3]);
        assert_eq!(vec![1, 2, 3], array![1, 2, 3]);
        assert_eq!(array![1, 2, 3], &[1, 2, 3][..]);
        assert_eq!(json!([1, 2, 3]), array![1, 2, 3].as_slice());
    }

    #[test]
    fn test_str_eq() {
        assert_eq!(json!("123"), FastStr::new("123"));
        assert_eq!(json!("123"), "123");
    }

    #[test]
    fn test_container_eq() {
        assert_eq!(json!([1, 2, 3]), array![1, 2, 3]);
        assert_eq!(array![1, 2, 3], json!([1, 2, 3]));
        assert_eq!(json!({"a": 1, "b": 2}), json!({"b": 2, "a": 1}));
        assert_eq!(json!({"a": 1, "b": 2}), object! {"a": 1, "b": 2});
        assert_eq!(object! {"a": 1, "b": 2}, json!({"a": 1, "b": 2}));
    }
}
