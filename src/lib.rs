#![cfg_attr(not(doctest), doc = include_str!("../README.md"))]
#![allow(dead_code)]
mod error;
mod input;
mod parser;
mod pointer;
mod reader;
mod util;

pub mod format;
pub mod lazyvalue;
pub mod serde;
pub mod value;
pub mod visitor;
pub mod writer;

#[doc(inline)]
pub use crate::error::{Error, Result};

#[doc(inline)]
pub use crate::lazyvalue::{
    get, get_from_bytes, get_from_bytes_unchecked, get_from_faststr, get_from_faststr_unchecked,
    get_from_slice, get_from_slice_unchecked, get_from_str, get_from_str_unchecked, get_many,
    get_many_unchecked, get_unchecked, to_array_iter, to_object_iter, ArrayIntoIter, LazyValue,
    ObjectIntoIter,
};

#[doc(inline)]
pub use crate::pointer::{JsonPointer, PointerNode, PointerTrait, PointerTree};

#[doc(inline)]
pub use crate::serde::{
    from_slice, from_slice_unchecked, from_str, to_raw_value, to_string, to_string_pretty, to_vec,
    to_vec_pretty, to_writer, to_writer_pretty, Deserializer, JsonNumberTrait, Number, RawNumber,
    RawValue, Serializer,
};

#[doc(inline)]
pub use crate::value::{
    dom_from_slice, dom_from_slice_unchecked, dom_from_str, Array, ArrayMut, Document, JsonType,
    JsonValue, Object, ObjectMut, Value, ValueMut,
};

// re-export the serde trait
pub use ::serde::{Deserialize, Serialize};
