#![cfg_attr(not(doctest), doc = include_str!("../README.md"))]
#![allow(clippy::needless_lifetimes)]
#![doc(test(attr(warn(unused))))]

mod config;
pub mod error;
mod index;
mod input;
mod pointer;
pub mod reader;
mod util;

pub mod format;
pub mod lazyvalue;
pub mod parser;
pub mod serde;
pub mod value;
pub mod writer;

// re-export FastStr
pub use ::faststr::FastStr;
// re-export the serde trait
pub use ::serde::{Deserialize, Serialize};
#[doc(inline)]
pub use reader::Read;

#[doc(inline)]
pub use crate::error::{Error, Result};
#[doc(inline)]
pub use crate::index::Index;
#[doc(inline)]
pub use crate::input::JsonInput;
#[doc(inline)]
pub use crate::lazyvalue::{
    get, get_from_bytes, get_from_bytes_unchecked, get_from_faststr, get_from_faststr_unchecked,
    get_from_slice, get_from_slice_unchecked, get_from_str, get_from_str_unchecked, get_many,
    get_many_unchecked, get_unchecked, to_array_iter, to_array_iter_unchecked, to_object_iter,
    to_object_iter_unchecked, ArrayJsonIter, LazyArray, LazyObject, LazyValue, ObjectJsonIter,
    OwnedLazyValue,
};
#[doc(inline)]
pub use crate::pointer::{JsonPointer, PointerNode, PointerTree};
#[doc(inline)]
pub use crate::serde::de::{MapAccess, SeqAccess};
#[doc(inline)]
pub use crate::serde::{
    from_reader, from_slice, from_slice_unchecked, from_str, to_lazyvalue, to_string,
    to_string_pretty, to_vec, to_vec_pretty, to_writer, to_writer_pretty, Deserializer,
    JsonNumberTrait, Number, RawNumber, Serializer, StreamDeserializer,
};
#[doc(inline)]
pub use crate::value::{
    from_value, get::get_by_schema, to_value, Array, JsonContainerTrait, JsonType,
    JsonValueMutTrait, JsonValueTrait, Object, Value, ValueRef,
};

pub mod prelude;
