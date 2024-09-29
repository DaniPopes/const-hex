#![allow(unsafe_op_in_unsafe_fn)]

use super::generic;
use crate::get_chars_table;
use core::arch::wasm32::*;

pub(crate) const USE_CHECK_FN: bool = false;

#[inline(always)]
fn has_simd128() -> bool {
    cfg!(target_feature = "simd128")
}

#[inline]
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: *mut u8) {
    if !has_simd128() {
        return generic::encode::<UPPER>(input, output);
    }
    encode_simd128::<UPPER>(input, output)
}

#[target_feature(enable = "simd128")]
unsafe fn encode_simd128<const UPPER: bool>(input: &[u8], output: *mut u8) {
    // Load table.
    let hex_table = v128_load(get_chars_table::<UPPER>().as_ptr().cast());

    generic::encode_unaligned_chunks::<UPPER, _>(input, output, |chunk: v128| {
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
        (hex_lo, hex_hi)
    });
}

#[inline]
pub(crate) fn check(input: &[u8]) -> bool {
    if !has_simd128() {
        return generic::check(input);
    }
    unsafe { check_simd128(input) }
}

#[target_feature(enable = "simd128")]
unsafe fn check_simd128(input: &[u8]) -> bool {
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

pub(crate) use generic::decode_checked;
pub(crate) use generic::decode_unchecked;
