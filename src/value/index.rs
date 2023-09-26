use crate::{value::Value, LazyValue};
use core::ops;

pub trait Index: private::Sealed {
    /// Return None if the index is not already in the array or object.
    #[doc(hidden)]
    fn value_index_into<'dom, 'v>(self, v: &'v Value<'dom>) -> Option<&'v Value<'dom>>;

    /// Return None if the index is not already in the array or object lazy_value.
    #[doc(hidden)]
    fn lazyvalue_index_into<'de>(self, v: &'de LazyValue<'de>) -> Option<LazyValue<'de>>;
}

pub trait IndexMut: private::Sealed {
    /// Return None if the key is not already in the array or object.
    #[doc(hidden)]
    fn index_into_mut<'dom, 'v>(self, v: &'v mut Value<'dom>) -> Option<&'v mut Value<'dom>>;
}

impl Index for usize {
    fn value_index_into<'dom, 'v>(self, v: &'v Value<'dom>) -> Option<&'v Value<'dom>> {
        v.get_index(self)
    }

    fn lazyvalue_index_into<'de>(self, v: &'de LazyValue<'de>) -> Option<LazyValue<'de>> {
        v.get_index(self)
    }
}

impl IndexMut for usize {
    fn index_into_mut<'dom, 'v>(self, v: &'v mut Value<'dom>) -> Option<&'v mut Value<'dom>> {
        v.get_index_mut(self)
    }
}

impl Index for &str {
    fn value_index_into<'dom, 'v>(self, v: &'v Value<'dom>) -> Option<&'v Value<'dom>> {
        v.get_key(self)
    }

    fn lazyvalue_index_into<'de>(self, v: &'de LazyValue<'de>) -> Option<LazyValue<'de>> {
        v.get_key(self)
    }
}

impl IndexMut for &str {
    fn index_into_mut<'dom, 'v>(self, v: &'v mut Value<'dom>) -> Option<&'v mut Value<'dom>> {
        v.get_key_mut(self)
    }
}

// Prevent users from implementing the Index trait.
mod private {
    pub trait Sealed {}
    impl Sealed for usize {}
    impl Sealed for str {}
    impl Sealed for std::string::String {}
    impl<'a, T> Sealed for &'a T where T: ?Sized + Sealed {}
}

impl<'dom, I> ops::Index<I> for Value<'dom>
where
    I: Index,
{
    type Output = Value<'dom>;

    fn index(&self, index: I) -> &Value<'dom> {
        // if not found, return NULL value
        thread_local! {
            pub static NULL: Value<'static> = const { Value::new_uinit() };
        }
        index.value_index_into(self).unwrap()
    }
}
