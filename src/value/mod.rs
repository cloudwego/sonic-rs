mod alloctor;
pub mod array;
pub(crate) mod de;
mod from;
pub(crate) mod node;
pub mod shared;
mod tryfrom;
#[macro_use]
mod macros;
pub mod object;
mod partial_eq;
mod ser;
mod value_trait;

#[doc(inline)]
pub use self::array::Array;
#[doc(inline)]
pub use self::de::from_value;
#[doc(inline)]
pub use self::node::{Value, ValueRef};
#[doc(inline)]
pub use self::object::Object;
#[doc(inline)]
pub use self::ser::{to_value, to_value_in};
#[doc(inline)]
pub use self::value_trait::{JsonContainerTrait, JsonType, JsonValueMutTrait, JsonValueTrait};
pub use crate::RawValue;

const MAX_STR_SIZE: usize = u32::MAX as usize;
const PTR_BITS: usize = 48;
