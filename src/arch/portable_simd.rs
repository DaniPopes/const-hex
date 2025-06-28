#![allow(unsafe_op_in_unsafe_fn)]

use super::generic;
use crate::get_chars_table;
use core::simd::prelude::*;

type Simd = u8x16;

pub(crate) const USE_CHECK_FN: bool = true;

pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: &mut [u8]) {
    // Load table.
    let hex_table = Simd::from_array(*get_chars_table::<UPPER>());

    generic::encode_unaligned_chunks::<UPPER, _, _>(input, output, |chunk: Simd| {
        // Load input bytes and mask to nibbles.
        let mut lo = chunk & Simd::splat(15);
        let mut hi = chunk >> Simd::splat(4);

        // Lookup the corresponding ASCII hex digit for each nibble.
        lo = hex_table.swizzle_dyn(lo);
        hi = hex_table.swizzle_dyn(hi);

        // Interleave the nibbles ([hi[0], lo[0], hi[1], lo[1], ...]).
        let (hex_lo, hex_hi) = Simd::interleave(hi, lo);
        [hex_lo, hex_hi]
    });
}

pub(crate) fn check(input: &[u8]) -> bool {
    generic::check_unaligned_chunks(input, |chunk: Simd| {
        let valid_digit = chunk.simd_ge(Simd::splat(b'0')) & chunk.simd_le(Simd::splat(b'9'));
        let valid_upper = chunk.simd_ge(Simd::splat(b'A')) & chunk.simd_le(Simd::splat(b'F'));
        let valid_lower = chunk.simd_ge(Simd::splat(b'a')) & chunk.simd_le(Simd::splat(b'f'));
        let valid = valid_digit | valid_upper | valid_lower;
        valid.all()
    })
}

// #[inline]
// pub(crate) unsafe fn decode_unchecked(input: &[u8], output: *mut u8) {
//     let offset_9 = Simd::splat(9);
//     let lower_mask = Simd::splat(0x20);

//     generic::decode_unchecked_unaligned_chunks::<Simd>(input, output, |a, b| {
//         // Convert to lowercase
//         let v_lower = v | lower_mask;

//         // Subtract '0' to normalize
//         let normalized = v_lower - Simd::splat(b'0');

//         // For digits (0-9): result is already correct
//         // For letters (49-54 after subtracting '0'): need to subtract 39 more
//         let is_alpha = normalized.simd_gt(offset_9);
//         let adjust = is_alpha.select(Simd::splat(39), Simd::splat(0));
//         normalized - adjust
//     });
// }
pub(crate) use generic::decode_unchecked;

// Not used.
pub(crate) use generic::decode_checked;
