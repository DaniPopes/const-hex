#![allow(unsafe_op_in_unsafe_fn)]

use super::generic;
use crate::{get_chars_table, Output};
use core::arch::wasm32::*;

pub(crate) const USE_CHECK_FN: bool = false;

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: impl Output) {
    // Load table.
    let hex_table = v128_load(get_chars_table::<UPPER>().as_ptr().cast());

    generic::encode_unaligned_chunks::<UPPER, _, _>(input, output, |chunk: v128| {
        // Load input bytes and mask to nibbles.
        let mut lo = v128_and(chunk, u8x16_splat(0x0F));
        let mut hi = u8x16_shr(chunk, 4);

        // Lookup the corresponding ASCII hex digit for each nibble.
        lo = u8x16_swizzle(hex_table, lo);
        hi = u8x16_swizzle(hex_table, hi);

        // Interleave the nibbles ([hi[0], lo[0], hi[1], lo[1], ...]).
        #[rustfmt::skip]
        let hex_lo = u8x16_shuffle::<
            0, 16,
            1, 17,
            2, 18,
            3, 19,
            4, 20,
            5, 21,
            6, 22,
            7, 23,
        >(hi, lo);
        #[rustfmt::skip]
        let hex_hi = u8x16_shuffle::<
            8, 24,
            9, 25,
            10, 26,
            11, 27,
            12, 28,
            13, 29,
            14, 30,
            15, 31,
        >(hi, lo);
        [hex_lo, hex_hi]
    });
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) fn check(input: &[u8]) -> bool {
    generic::check_unaligned_chunks(input, |chunk: v128| {
        let ge0 = u8x16_ge(chunk, u8x16_splat(b'0'));
        let le9 = u8x16_le(chunk, u8x16_splat(b'9'));
        let valid_digit = v128_and(ge0, le9);

        let geua = u8x16_ge(chunk, u8x16_splat(b'A'));
        let leuf = u8x16_le(chunk, u8x16_splat(b'F'));
        let valid_upper = v128_and(geua, leuf);

        let gela = u8x16_ge(chunk, u8x16_splat(b'a'));
        let lelf = u8x16_le(chunk, u8x16_splat(b'f'));
        let valid_lower = v128_and(gela, lelf);

        let valid_letter = v128_or(valid_lower, valid_upper);
        let valid = v128_or(valid_digit, valid_letter);
        u8x16_all_true(valid)
    })
}

/// Single-pass hex decode with validation using Muła & Langdale's Algorithm #3.
///
/// Converts ASCII hex to nibble values and validates simultaneously:
/// - Digits '0'..'9' → 0..9, letters 'A'..'F'/'a'..'f' → 10..15 via saturation arithmetic.
/// - Invalid bytes produce values > 15, detected via `u8x16_add_sat(nibble, 112)` setting the MSB.
/// - Nibble pairs are merged with `u8x16_shuffle` deinterleave + `(hi << 4) | lo`.
///
/// Based on: <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [u8]) -> bool {
    debug_assert_eq!(output.len(), input.len() / 2);

    let add_c6 = u8x16_splat(0xC6); // 0xFF - b'9'
    let six = u8x16_splat(6);
    let f0 = u8x16_splat(0xF0);
    let df = u8x16_splat(0xDF);
    let big_a = u8x16_splat(b'A');
    let ten = u8x16_splat(10);
    let check_bias = u8x16_splat(112); // 127 - 15

    generic::decode_checked_unaligned_chunks(input, output, |[v0, v1]: [v128; 2]| {
        // Digits '0'..'9' → 0..9, others > 15.
        let d0 = u8x16_sub(u8x16_sub_sat(u8x16_add(v0, add_c6), six), f0);
        let d1 = u8x16_sub(u8x16_sub_sat(u8x16_add(v1, add_c6), six), f0);

        // Letters 'A'..'F'/'a'..'f' → 10..15, others > 15.
        let a0 = u8x16_add_sat(u8x16_sub(v128_and(v0, df), big_a), ten);
        let a1 = u8x16_add_sat(u8x16_sub(v128_and(v1, df), big_a), ten);

        // Valid nibble wins (0..15), invalid stays > 15.
        let n0 = u8x16_min(d0, a0);
        let n1 = u8x16_min(d1, a1);

        // Validate: saturating add sets MSB if nibble > 15.
        let c = v128_or(u8x16_add_sat(n0, check_bias), u8x16_add_sat(n1, check_bias));
        if u8x16_bitmask(c) != 0 {
            return None;
        }

        // Deinterleave and merge nibble pairs.
        #[rustfmt::skip]
        let hi = u8x16_shuffle::<0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30>(n0, n1);
        #[rustfmt::skip]
        let lo = u8x16_shuffle::<1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31>(n0, n1);
        Some(v128_or(u8x16_shl(hi, 4), lo))
    })
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: &mut [u8]) {
    generic::decode_unchecked_unaligned_chunks(input, output, |[v0, v1]: [v128; 2]| {
        let n0 = unhex(v0);
        let n1 = unhex(v1);

        // Deinterleave and merge nibble pairs.
        #[rustfmt::skip]
        let hi = u8x16_shuffle::<0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30>(n0, n1);
        #[rustfmt::skip]
        let lo = u8x16_shuffle::<1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31>(n0, n1);
        v128_or(u8x16_shl(hi, 4), lo)
    });
}

/// Converts ASCII hex bytes to nibble values: `(x >> 6) * 9 + (x & 0x0F)`.
#[inline]
#[target_feature(enable = "simd128")]
unsafe fn unhex(x: v128) -> v128 {
    let sr6 = u8x16_shr(x, 6);
    let low = v128_and(x, u8x16_splat(0x0F));
    // sr6 * 9 = (sr6 << 3) + sr6 (no i8x16.mul in wasm SIMD).
    let mul9 = u8x16_add(u8x16_shl(sr6, 3), sr6);
    u8x16_add(mul9, low)
}
