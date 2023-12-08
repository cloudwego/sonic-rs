mod get;
mod iterator;
mod value;

pub use get::{
    get, get_from_bytes, get_from_bytes_unchecked, get_from_faststr, get_from_faststr_unchecked,
    get_from_slice, get_from_slice_unchecked, get_from_str, get_from_str_unchecked, get_many,
    get_many_unchecked, get_unchecked,
};
pub use iterator::{
    to_array_iter, to_array_iter_unchecked, to_object_iter, to_object_iter_unchecked,
    ArrayIntoIter, ObjectIntoIter, UnsafeArrayIntoIter, UnsafeObjectIntoIter,
};
pub use value::LazyValue;

pub(crate) mod de;
pub(crate) mod ser;

pub(crate) const TOKEN: &str = "$sonic_rs::LazyValue";
