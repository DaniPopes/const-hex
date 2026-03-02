#![allow(unsafe_op_in_unsafe_fn)]

use super::generic;
use crate::{get_chars_table, Output};
use core::arch::aarch64::*;

pub(crate) const USE_CHECK_FN: bool = false;

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
    if !has_neon() {
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
    if !has_neon() {
        return generic::check(input);
    }
    unsafe { check_neon(input) }
}

#[target_feature(enable = "neon")]
pub(crate) unsafe fn check_neon(input: &[u8]) -> bool {
    generic::check_unaligned_chunks(input, |chunk: uint8x16_t| {
        let ge0 = vcgeq_u8(chunk, vdupq_n_u8(b'0'));
        let le9 = vcleq_u8(chunk, vdupq_n_u8(b'9'));
        let valid_digit = vandq_u8(ge0, le9);

        let geua = vcgeq_u8(chunk, vdupq_n_u8(b'A'));
        let leuf = vcleq_u8(chunk, vdupq_n_u8(b'F'));
        let valid_upper = vandq_u8(geua, leuf);

        let gela = vcgeq_u8(chunk, vdupq_n_u8(b'a'));
        let lelf = vcleq_u8(chunk, vdupq_n_u8(b'f'));
        let valid_lower = vandq_u8(gela, lelf);

        let valid_letter = vorrq_u8(valid_lower, valid_upper);
        let valid_mask = vorrq_u8(valid_digit, valid_letter);
        vminvq_u8(valid_mask) == 0xFF
    })
}

/// Single-pass hex decode with validation using Muła & Langdale's Algorithm #3.
///
/// Converts ASCII hex to nibble values and validates simultaneously:
/// - Digits '0'..'9' → 0..9, letters 'A'..'F'/'a'..'f' → 10..15 via saturation arithmetic.
/// - Invalid bytes produce values > 15, detected via `adds(nibble, 112)` setting the MSB.
/// - Nibble pairs are merged with `vuzpq_u8` deinterleave + `(hi << 4) | lo`.
///
/// Based on: <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
#[inline]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [u8]) -> bool {
    if cfg!(miri) || !has_neon() {
        return generic::decode_checked(input, output);
    }
    decode_checked_neon(input, output)
}

#[target_feature(enable = "neon")]
unsafe fn decode_checked_neon(input: &[u8], output: &mut [u8]) -> bool {
    debug_assert_eq!(output.len(), input.len() / 2);

    let add_c6 = vdupq_n_u8(0xC6); // 0xFF - b'9'
    let six = vdupq_n_u8(6);
    let f0 = vdupq_n_u8(0xF0);
    let df = vdupq_n_u8(0xDF);
    let big_a = vdupq_n_u8(b'A');
    let ten = vdupq_n_u8(10);
    let check_bias = vdupq_n_u8(112); // 127 - 15

    generic::decode_checked_unaligned_chunks(input, output, |[v0, v1]: [uint8x16_t; 2]| {
        // Digits '0'..'9' → 0..9, others > 15.
        let d0 = vsubq_u8(vqsubq_u8(vaddq_u8(v0, add_c6), six), f0);
        let d1 = vsubq_u8(vqsubq_u8(vaddq_u8(v1, add_c6), six), f0);

        // Letters 'A'..'F'/'a'..'f' → 10..15, others > 15.
        let a0 = vqaddq_u8(vsubq_u8(vandq_u8(v0, df), big_a), ten);
        let a1 = vqaddq_u8(vsubq_u8(vandq_u8(v1, df), big_a), ten);

        // Valid nibble wins (0..15), invalid stays > 15.
        let n0 = vminq_u8(d0, a0);
        let n1 = vminq_u8(d1, a1);

        // Validate: saturating add sets MSB if nibble > 15.
        let c = vorrq_u8(vqaddq_u8(n0, check_bias), vqaddq_u8(n1, check_bias));
        if vmaxvq_u8(c) > 0x7F {
            return None;
        }

        // Merge nibble pairs.
        let uz = vuzpq_u8(n0, n1);
        Some(vorrq_u8(vshlq_n_u8(uz.0, 4), uz.1))
    })
}

#[inline]
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: &mut [u8]) {
    if cfg!(miri) || !has_neon() {
        return generic::decode_unchecked(input, output);
    }
    decode_unchecked_neon(input, output);
}

#[target_feature(enable = "neon")]
unsafe fn decode_unchecked_neon(input: &[u8], output: &mut [u8]) {
    generic::decode_unchecked_unaligned_chunks(input, output, |[v0, v1]: [uint8x16_t; 2]| {
        let n0 = unhex_neon(v0);
        let n1 = unhex_neon(v1);
        let uz = vuzpq_u8(n0, n1);
        vorrq_u8(vshlq_n_u8(uz.0, 4), uz.1)
    });
}

/// Converts ASCII hex bytes to nibble values: `(x >> 6) * 9 + (x & 0x0F)`.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn unhex_neon(x: uint8x16_t) -> uint8x16_t {
    let sr6 = vshrq_n_u8(x, 6);
    let low = vandq_u8(x, vdupq_n_u8(0x0F));
    let mul9 = vmulq_u8(sr6, vdupq_n_u8(9));
    vaddq_u8(mul9, low)
}
