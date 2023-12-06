pub use crate::RawValue;
pub mod node;
pub use node::{Value, ValueRef};

pub use value_trait::{JsonContainerTrait, JsonType, JsonValueMutTrait, JsonValueTrait};
pub mod alloctor;
pub mod array;
pub mod de;
mod from;
pub mod index;
pub mod shared;
#[macro_use]
mod macros;
pub mod object;
mod partial_eq;
pub mod ser;
pub mod value_trait;
pub use array::Array;
pub use object::Object;
mod tryfrom;

pub use ser::to_value;

pub use de::from_value;

const MAX_STR_SIZE: usize = u32::MAX as usize;
const PTR_BITS: usize = 48;
