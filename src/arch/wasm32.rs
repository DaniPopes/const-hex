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

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [u8]) -> bool {
    let input_len = input.len();
    let mut i = 0;
    let mut o = 0;

    while i + 32 <= input_len {
        let chunk0 = v128_load(input.as_ptr().add(i).cast());
        let chunk1 = v128_load(input.as_ptr().add(i + 16).cast());

        #[rustfmt::skip]
        let hi_chars = u8x16_shuffle::<
            0, 2, 4, 6, 8, 10, 12, 14,
            16, 18, 20, 22, 24, 26, 28, 30,
        >(chunk0, chunk1);
        #[rustfmt::skip]
        let lo_chars = u8x16_shuffle::<
            1, 3, 5, 7, 9, 11, 13, 15,
            17, 19, 21, 23, 25, 27, 29, 31,
        >(chunk0, chunk1);

        let (hi_nib, hi_valid) = unhex_wasm(hi_chars);
        let (lo_nib, lo_valid) = unhex_wasm(lo_chars);

        let valid = v128_and(hi_valid, lo_valid);
        if !u8x16_all_true(valid) {
            return false;
        }

        let decoded = v128_or(u8x16_shl(hi_nib, 4), lo_nib);
        v128_store(output.as_mut_ptr().add(o).cast(), decoded);

        i += 32;
        o += 16;
    }

    if i < input_len {
        if !generic::decode_checked(&input[i..], &mut output[o..]) {
            return false;
        }
    }

    true
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: &mut [u8]) {
    let success = decode_checked(input, output);
    debug_assert!(success);
}

#[inline(always)]
fn unhex_wasm(chars: v128) -> (v128, v128) {
    let zero = u8x16_splat(b'0');
    let nine = u8x16_splat(b'9');
    let ascii_a = u8x16_splat(b'a');
    let ascii_f = u8x16_splat(b'f');
    let case_mask = u8x16_splat(0x20);

    let digit_val = u8x16_sub(chars, zero);
    let is_digit = v128_and(u8x16_ge(chars, zero), u8x16_le(chars, nine));

    let lower = v128_or(chars, case_mask);
    let letter_val = u8x16_add(u8x16_sub(lower, ascii_a), u8x16_splat(10));
    let is_letter = v128_and(u8x16_ge(lower, ascii_a), u8x16_le(lower, ascii_f));

    let nibble = v128_bitselect(digit_val, letter_val, is_digit);
    let valid = v128_or(is_digit, is_letter);
    (nibble, valid)
}
