//! A dynamic type to representing any valid JSON value.

pub mod array;
pub(crate) mod de;
mod from;
pub(crate) mod node;
#[doc(hidden)]
pub mod shared;
mod tryfrom;
#[macro_use]
mod macros;
pub mod object;
mod partial_eq;
mod ser;
mod tls_buffer;
mod value_trait;
pub(crate) mod visitor;

#[doc(inline)]
pub use self::array::Array;
#[doc(inline)]
pub use self::de::from_value;
#[doc(inline)]
pub use self::node::{Value, ValueRef};
#[doc(inline)]
pub use self::object::Object;
#[doc(inline)]
pub use self::ser::to_value;
#[doc(inline)]
pub use self::value_trait::{JsonContainerTrait, JsonType, JsonValueMutTrait, JsonValueTrait};

const MAX_STR_SIZE: usize = u32::MAX as usize;
const PTR_BITS: usize = 48;
