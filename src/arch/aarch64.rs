#![allow(unsafe_op_in_unsafe_fn)]

use super::generic;
use crate::{get_chars_table, Output};
use core::arch::aarch64::*;

pub(crate) const USE_CHECK_FN: bool = true;

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        #[inline(always)]
        fn has_neon() -> bool {
            std::arch::is_aarch64_feature_detected!("neon")
        }
    } else {
        #[inline(always)]
        fn has_neon() -> bool {
            cfg!(target_feature = "neon")
        }
    }
}

#[inline]
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: impl Output) {
    if cfg!(miri) || !has_neon() {
        return generic::encode::<UPPER>(input, output);
    }
    encode_neon::<UPPER>(input, output);
}

#[target_feature(enable = "neon")]
pub(crate) unsafe fn encode_neon<const UPPER: bool>(input: &[u8], output: impl Output) {
    // Load table.
    let hex_table = vld1q_u8(get_chars_table::<UPPER>().as_ptr());

    generic::encode_unaligned_chunks::<UPPER, _, _>(input, output, |chunk: uint8x16_t| {
        // Load input bytes and mask to nibbles.
        let mut lo = vandq_u8(chunk, vdupq_n_u8(0x0F));
        let mut hi = vshrq_n_u8(chunk, 4);

        // Lookup the corresponding ASCII hex digit for each nibble.
        lo = vqtbl1q_u8(hex_table, lo);
        hi = vqtbl1q_u8(hex_table, hi);

        // Interleave the nibbles ([hi[0], lo[0], hi[1], lo[1], ...]).
        vzipq_u8(hi, lo)
    });
}

#[inline]
pub(crate) fn check(input: &[u8]) -> bool {
    if cfg!(miri) || !has_neon() {
        return generic::check(input);
    }
    unsafe { check_neon(input) }
}

/// Hex check using signed overflow trick: bias each valid range so it starts at `i8::MIN`,
/// then a single `cmpgt(start + len, x)` checks if `x` falls within the range.
///
/// - Digits '0'..'9' (0x30..0x39): bias by 0xB0 maps to -128..-119, threshold -118 (10 values).
/// - Letters 'A'..'F' (0x41..0x46): bias by 0xC1 maps to -128..-123, threshold -122 (6 values).
///   Case folded with 0xDF mask so 'a'..'f' is handled identically.
///
/// Based on Muła & Langdale:
/// <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
#[target_feature(enable = "neon")]
pub(crate) unsafe fn check_neon(input: &[u8]) -> bool {
    let digit_bias = vdupq_n_s8(0xB0_u8 as i8);
    let alpha_bias = vdupq_n_s8(0xC1_u8 as i8);
    let case_mask = vdupq_n_u8(0xDF);
    let digit_threshold = vdupq_n_s8(-118); // i8::MIN + 10
    let alpha_threshold = vdupq_n_s8(-122); // i8::MIN + 6

    generic::check_unaligned_chunks(input, |chunk: uint8x16_t| {
        let chunk_s = vreinterpretq_s8_u8(chunk);

        let x1 = vsubq_s8(chunk_s, digit_bias);
        let m1 = vcgtq_s8(digit_threshold, x1);

        let folded = vreinterpretq_s8_u8(vandq_u8(chunk, case_mask));
        let x2 = vsubq_s8(folded, alpha_bias);
        let m2 = vcgtq_s8(alpha_threshold, x2);

        let valid = vorrq_u8(m1, m2);
        vminvq_u8(valid) == 0xFF
    })
}

pub(crate) use generic::decode_checked;
pub(crate) use generic::decode_unchecked;
