// Copyright 2018-2019 The simdjson authors

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at

//     http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// This file may have been modified by ByteDance authors. All ByteDance
// Modifications are Copyright 2022 ByteDance Authors.

use std::arch::aarch64::*;

// Not use PMULL instructions, but it is apparently slow.
// This is copied from simdjson.
pub unsafe fn prefix_xor(bitmask: u64) -> u64 {
    let mut bitmask = bitmask;
    bitmask ^= bitmask << 1;
    bitmask ^= bitmask << 2;
    bitmask ^= bitmask << 4;
    bitmask ^= bitmask << 8;
    bitmask ^= bitmask << 16;
    bitmask ^= bitmask << 32;
    bitmask
}

// We compute whitespace and op separately. If the code later only use one or the
// other, given the fact that all functions are aggressively inlined, we can
// hope that useless computations will be omitted. This is namely case when
// minifying (we only need whitespace). *However* if we only need spaces,
// it is likely that we will still compute 'v' above with two lookup_16: one
// could do it a bit cheaper. This is in contrast with the x64 implementations
// where we can, efficiently, do the white space and structural matching
// separately. One reason for this difference is that on ARM NEON, the table
// lookups either zero or leave unchanged the characters exceeding 0xF whereas
// on x64, the equivalent instruction (pshufb) automatically applies a mask,
// ignoring the 4 most significant bits. Thus the x64 implementation is
// optimized differently. This being said, if you use this code strictly
// just for minification (or just to identify the structural characters),
// there is a small untaken optimization opportunity here. We deliberately
// do not pick it up.
#[inline(always)]
pub unsafe fn get_nonspace_bits(data: &[u8; 64]) -> u64 {
    // return super::fallback::get_nonspace_bits(data);
    #[inline(always)]
    unsafe fn chunk_nonspace_bits(input: uint8x16_t) -> uint8x16_t {
        const LOW_TAB: uint8x16_t =
            unsafe { std::mem::transmute([16u8, 0, 0, 0, 0, 0, 0, 0, 0, 8, 12, 1, 2, 9, 0, 0]) };

        const HIGH_TAB: uint8x16_t =
            unsafe { std::mem::transmute([8u8, 0, 18, 4, 0, 1, 0, 1, 0, 0, 0, 3, 2, 1, 0, 0]) };

        let white_mask = vmovq_n_u8(0x18);
        let lo4 = vandq_u8(input, vmovq_n_u8(0xf));
        let hi4 = vshrq_n_u8(input, 4);

        let lo4_sf = vqtbl1q_u8(LOW_TAB, lo4);
        let hi4_sf = vqtbl1q_u8(HIGH_TAB, hi4);

        let v = vandq_u8(lo4_sf, hi4_sf);

        vtstq_u8(v, white_mask)
    }

    !sonic_simd::neon::to_bitmask64(
        chunk_nonspace_bits(vld1q_u8(data.as_ptr())),
        chunk_nonspace_bits(vld1q_u8(data.as_ptr().offset(16))),
        chunk_nonspace_bits(vld1q_u8(data.as_ptr().offset(32))),
        chunk_nonspace_bits(vld1q_u8(data.as_ptr().offset(48))),
    )
}
