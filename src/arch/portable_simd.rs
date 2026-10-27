#![allow(unsafe_op_in_unsafe_fn)]

use super::generic;
use crate::{Output, get_chars_table};
use core::mem::MaybeUninit;
use core::simd::prelude::*;

type Simd = u8x16;

pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: impl Output) {
    // Load table.
    let hex_table = Simd::from_array(*get_chars_table::<UPPER>());

    generic::encode_unaligned_chunks::<UPPER, 2, _, _>(input, output, |chunk: Simd| {
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
    generic::check_unaligned_chunks(
        input,
        |chunk: Simd| {
            let valid_digit = chunk.simd_ge(Simd::splat(b'0')) & chunk.simd_le(Simd::splat(b'9'));
            let valid_upper = chunk.simd_ge(Simd::splat(b'A')) & chunk.simd_le(Simd::splat(b'F'));
            let valid_lower = chunk.simd_ge(Simd::splat(b'a')) & chunk.simd_le(Simd::splat(b'f'));
            valid_digit | valid_upper | valid_lower
        },
        |a, b| a & b,
        |mask| mask.all(),
    )
}

/// Single-pass hex decode with validation using Muła & Langdale's Algorithm #3.
///
/// Converts ASCII hex to nibble values and validates simultaneously:
/// - Digits '0'..'9' → 0..9, letters 'A'..'F'/'a'..'f' → 10..15 via saturation arithmetic.
/// - Invalid bytes produce values > 15, detected via `saturating_add(nibble, 112)` setting the MSB.
/// - Nibble pairs are merged with `deinterleave` + `(hi << 4) | lo`.
///
/// Based on: <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    debug_assert_eq!(output.len(), input.len() / 2);

    let add_c6 = Simd::splat(0xC6); // 0xFF - b'9'
    let six = Simd::splat(6);
    let f0 = Simd::splat(0xF0);
    let df = Simd::splat(0xDF);
    let big_a = Simd::splat(b'A');
    let ten = Simd::splat(10);
    let check_bias = Simd::splat(112); // 127 - 15

    generic::decode_checked_unaligned_chunks(
        input,
        output,
        |v: Simd| {
            // Digits '0'..'9' → 0..9, others > 15.
            let d = (v + add_c6).saturating_sub(six) - f0;

            // Letters 'A'..'F'/'a'..'f' → 10..15, others > 15.
            let a = ((v & df) - big_a).saturating_add(ten);

            // Valid nibble wins (0..15), invalid stays > 15.
            d.simd_min(a)
        },
        |a, b| a | b,
        // Saturating add sets MSB if nibble > 15.
        |nibbles| {
            !nibbles
                .saturating_add(check_bias)
                .simd_gt(Simd::splat(0x7F))
                .any()
        },
        |n0, n1| {
            // Deinterleave and merge nibble pairs.
            let (hi, lo) = Simd::deinterleave(n0, n1);
            (hi << Simd::splat(4)) | lo
        },
    )
}
