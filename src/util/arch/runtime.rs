//! Runtime CPU feature detection for x86_64.
//!
//! This module is only compiled when targeting x86_64 without compile-time AVX2/PCLMULQDQ
//! features. It uses the `multiversion` crate to detect CPU features at runtime and dispatch
//! to either the optimized x86_64 implementations (AVX2/PCLMULQDQ) or the scalar fallback.
//!
//! When the features *are* known at compile time (e.g. via `-C target-cpu=native`), this
//! module is not compiled at all — the optimized implementations are used directly with
//! zero overhead. See the `cfg_if` dispatch in `mod.rs`.

use multiversion::multiversion;
use multiversion::target::match_target;

/// Detect non-whitespace bytes in a 64-byte block and return a bitmask.
///
/// At runtime, dispatches to:
/// - AVX2 shuffle-based implementation if the CPU supports AVX2
/// - Scalar byte-by-byte fallback otherwise
///
/// The `multiversion` macro compiles this function once per listed target. The `match_target!`
/// macro resolves at compile time *within each clone*, selecting the appropriate implementation.
/// After the first call the selected function pointer is cached in a static atomic — subsequent
/// calls are a single atomic load + indirect call (~1-2 ns overhead).
#[multiversion(targets("x86_64+avx2", "x86_64+sse2"))]
pub unsafe fn get_nonspace_bits(data: &[u8; 64]) -> u64 {
    match_target! {
        "x86_64+avx2" => super::x86_64::get_nonspace_bits(data),
        _ => super::fallback::get_nonspace_bits(data),
    }
}

/// Compute prefix XOR of a 64-bit bitmask.
///
/// At runtime, dispatches to:
/// - PCLMULQDQ carryless-multiply implementation if the CPU supports it
/// - Scalar shift-cascade fallback otherwise
#[multiversion(targets("x86_64+pclmulqdq", "x86_64+sse2"))]
pub unsafe fn prefix_xor(bitmask: u64) -> u64 {
    match_target! {
        "x86_64+pclmulqdq" => super::x86_64::prefix_xor(bitmask),
        _ => super::fallback::prefix_xor(bitmask),
    }
}
