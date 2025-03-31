//! Imports the various traits about JSON. `use sonic_rs::prelude::*` to make the
//! various traits and methods imported if you need.

pub use crate::{
    index::Index,
    input::JsonInput,
    reader::{Read, Reader},
    serde::JsonNumberTrait,
    value::{JsonContainerTrait, JsonValueMutTrait, JsonValueTrait},
    writer::WriteExt,
};
