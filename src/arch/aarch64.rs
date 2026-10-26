#![allow(unsafe_op_in_unsafe_fn)]

use super::generic;
use crate::{Output, get_chars_table};
use core::arch::aarch64::*;
use core::mem::MaybeUninit;

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

#[inline]
#[target_feature(enable = "neon")]
pub(crate) unsafe fn encode_neon<const UPPER: bool>(input: &[u8], mut output: impl Output) {
    let table = vld1q_u8(get_chars_table::<UPPER>().as_ptr());
    let len = input.len();
    let ip = input.as_ptr();
    let op = output.as_mut_ptr();

    if len >= 16 {
        let mut o = op;
        for chunk in input.chunks_exact(16) {
            encode_16(table, chunk.as_ptr(), o);
            o = o.add(32);
        }
        // Encode the last 16 bytes again, overlapping what was already written.
        if len % 16 != 0 {
            encode_16(table, ip.add(len - 16), op.add((len - 16) * 2));
        }
    } else if len >= 8 {
        // Two overlapping 8-byte loads; each 16-character half is stored at its own offset.
        let v = vcombine_u8(vld1_u8(ip), vld1_u8(ip.add(len - 8)));
        let (hi, lo) = chars(table, v);
        let z = vzipq_u8(hi, lo);
        vst1q_u8(op, z.0);
        vst1q_u8(op.add((len - 8) * 2), z.1);
    } else if len >= 4 {
        // Two overlapping 4-byte loads; each 8-character half is stored at its own offset.
        let first = ip.cast::<u32>().read_unaligned() as u64;
        let last = ip.add(len - 4).cast::<u32>().read_unaligned() as u64;
        let v = vcreate_u8(first | (last << 32));
        let (hi, lo) = chars(table, vcombine_u8(v, v));
        let z = vreinterpretq_u64_u8(vzip1q_u8(hi, lo));
        op.cast::<u64>().write_unaligned(vgetq_lane_u64::<0>(z));
        op.add((len - 4) * 2)
            .cast::<u64>()
            .write_unaligned(vgetq_lane_u64::<1>(z));
    } else {
        generic::encode::<UPPER>(input, output);
    }
}

/// Returns the hex characters of the high and low nibbles of each byte in `v`.
#[inline(always)]
unsafe fn chars(table: uint8x16_t, v: uint8x16_t) -> (uint8x16_t, uint8x16_t) {
    let hi = vqtbl1q_u8(table, vshrq_n_u8(v, 4));
    let lo = vqtbl1q_u8(table, vandq_u8(v, vdupq_n_u8(0x0F)));
    (hi, lo)
}

/// Encodes 16 bytes at `ip` to 32 characters at `op`.
#[inline(always)]
unsafe fn encode_16(table: uint8x16_t, ip: *const u8, op: *mut u8) {
    let (hi, lo) = chars(table, vld1q_u8(ip));
    let z = vzipq_u8(hi, lo);
    vst1q_u8(op, z.0);
    vst1q_u8(op.add(16), z.1);
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
/// - Invalid bytes produce values > 15, detected via `vqaddq_u8(nibble, 112)` setting the MSB.
/// - Nibble pairs are merged with `vuzpq_u8` deinterleave + `(hi << 4) | lo`.
///
/// Based on: <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
#[inline]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    if cfg!(miri) || !has_neon() {
        return generic::decode_checked(input, output);
    }
    decode_checked_neon(input, output)
}

#[target_feature(enable = "neon")]
unsafe fn decode_checked_neon(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
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
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: &mut [MaybeUninit<u8>]) {
    if cfg!(miri) || !has_neon() {
        return generic::decode_unchecked(input, output);
    }
    decode_unchecked_neon(input, output);
}

#[target_feature(enable = "neon")]
unsafe fn decode_unchecked_neon(input: &[u8], output: &mut [MaybeUninit<u8>]) {
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
