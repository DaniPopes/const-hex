#![allow(unsafe_op_in_unsafe_fn)]

use super::generic;
use crate::{Output, get_chars_table};
use core::arch::wasm32::*;
use core::mem::MaybeUninit;

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

/// Converts ASCII hex to nibbles using Muła & Langdale's Algorithm #3: valid bytes become
/// `0..=15`, invalid bytes become values above 15.
///
/// Based on: <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
#[inline]
#[target_feature(enable = "simd128")]
unsafe fn nibbles(v: v128) -> v128 {
    // Digits '0'..'9' → 0..9, others > 15.
    let d = u8x16_sub(
        u8x16_sub_sat(u8x16_add(v, u8x16_splat(0xC6)), u8x16_splat(6)),
        u8x16_splat(0xF0),
    );
    // Letters 'A'..'F'/'a'..'f' → 10..15, others > 15.
    let a = u8x16_add_sat(
        u8x16_sub(v128_and(v, u8x16_splat(0xDF)), u8x16_splat(b'A')),
        u8x16_splat(10),
    );
    // Valid nibble wins (0..15), invalid stays > 15.
    u8x16_min(d, a)
}

/// Merges 32 nibbles into 16 bytes by deinterleaving and combining each pair.
#[inline]
#[target_feature(enable = "simd128")]
unsafe fn merge(n0: v128, n1: v128) -> v128 {
    #[rustfmt::skip]
    let hi = u8x16_shuffle::<0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30>(n0, n1);
    #[rustfmt::skip]
    let lo = u8x16_shuffle::<1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31>(n0, n1);
    // The nibbles are at most 15, so the 16-bit shift cannot carry into the next byte.
    v128_or(i16x8_shl(hi, 4), lo)
}

/// Returns whether any nibble is above 15, which sets a bit in `0xF0`.
#[inline]
#[target_feature(enable = "simd128")]
unsafe fn any_invalid(n: v128) -> bool {
    v128_any_true(v128_and(n, u8x16_splat(0xF0)))
}

/// Decodes 32 hex characters into 16 bytes, or returns `None` if any character is invalid.
#[inline]
#[target_feature(enable = "simd128")]
unsafe fn decode32(v0: v128, v1: v128) -> Option<v128> {
    let n0 = nibbles(v0);
    let n1 = nibbles(v1);
    if any_invalid(v128_or(n0, n1)) {
        return None;
    }
    Some(merge(n0, n1))
}

/// Single-pass hex decode with validation.
///
/// The main loop decodes 64 characters per iteration. Inputs of 8 or more characters use
/// overlapping loads and stores, so no scalar tail is needed.
#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    debug_assert_eq!(output.len(), input.len() / 2);

    let p = input.as_ptr();
    let o = output.as_mut_ptr().cast::<u8>();
    let n = input.len();
    let m = output.len();

    if n >= 32 {
        let mut i = 0;
        while i + 64 <= n {
            let q = p.add(i).cast::<v128>();
            let n0 = nibbles(v128_load(q));
            let n1 = nibbles(v128_load(q.add(1)));
            let n2 = nibbles(v128_load(q.add(2)));
            let n3 = nibbles(v128_load(q.add(3)));
            if any_invalid(v128_or(v128_or(n0, n1), v128_or(n2, n3))) {
                return false;
            }
            v128_store(o.add(i / 2).cast(), merge(n0, n1));
            v128_store(o.add(i / 2 + 16).cast(), merge(n2, n3));
            i += 64;
        }
        if i + 32 <= n {
            let q = p.add(i).cast::<v128>();
            let Some(x) = decode32(v128_load(q), v128_load(q.add(1))) else {
                return false;
            };
            v128_store(o.add(i / 2).cast(), x);
            i += 32;
        }
        // Decode the last 32 characters again to cover the remainder.
        if i < n {
            let q = p.add(n - 32).cast::<v128>();
            let Some(x) = decode32(v128_load(q), v128_load(q.add(1))) else {
                return false;
            };
            v128_store(o.add(m - 16).cast(), x);
        }
        return true;
    }

    if n >= 16 {
        let Some(x) = decode32(v128_load(p.cast()), v128_load(p.add(n - 16).cast())) else {
            return false;
        };
        v128_store64_lane::<0>(x, o.cast());
        v128_store64_lane::<1>(x, o.add(m - 8).cast());
        return true;
    }

    if n >= 8 {
        let v = v128_load64_lane::<1>(v128_load64_zero(p.cast()), p.add(n - 8).cast());
        let Some(x) = decode32(v, v) else {
            return false;
        };
        v128_store32_lane::<0>(x, o.cast());
        v128_store32_lane::<1>(x, o.add(m - 4).cast());
        return true;
    }

    generic::decode_checked(input, output)
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: &mut [MaybeUninit<u8>]) {
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
