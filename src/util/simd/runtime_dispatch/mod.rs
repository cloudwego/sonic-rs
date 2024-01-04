mod d128;
mod d256;

pub use self::{d128::*, d256::*};
// pick v512 simd
// TODO: support avx512?
pub use super::v512::*;
