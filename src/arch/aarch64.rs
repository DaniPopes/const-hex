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

#[inline]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [u8]) -> bool {
    if cfg!(miri) || !has_neon() {
        return generic::decode_checked(input, output);
    }
    unsafe { decode_checked_neon(input, output) }
}

#[inline]
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: &mut [u8]) {
    if cfg!(miri) || !has_neon() {
        return generic::decode_unchecked(input, output);
    }
    let success = unsafe { decode_checked_neon(input, output) };
    debug_assert!(success);
}

#[target_feature(enable = "neon")]
unsafe fn decode_checked_neon(input: &[u8], output: &mut [u8]) -> bool {
    let input_len = input.len();
    let mut i = 0;
    let mut o = 0;

    while i + 32 <= input_len {
        let chunk0 = vld1q_u8(input.as_ptr().add(i));
        let chunk1 = vld1q_u8(input.as_ptr().add(i + 16));

        let hi_chars = vuzp1q_u8(chunk0, chunk1);
        let lo_chars = vuzp2q_u8(chunk0, chunk1);

        let (hi_nib, hi_valid) = unhex_neon(hi_chars);
        let (lo_nib, lo_valid) = unhex_neon(lo_chars);

        let valid = vandq_u8(hi_valid, lo_valid);
        if vminvq_u8(valid) != 0xFF {
            return false;
        }

        let decoded = vorrq_u8(vshlq_n_u8(hi_nib, 4), lo_nib);
        vst1q_u8(output.as_mut_ptr().add(o), decoded);

        i += 32;
        o += 16;
    }

    if i < input_len {
        let rem_input = &input[i..];
        let rem_output = &mut output[o..];
        if !generic::decode_checked(rem_input, rem_output) {
            return false;
        }
    }

    true
}

#[inline(always)]
unsafe fn unhex_neon(chars: uint8x16_t) -> (uint8x16_t, uint8x16_t) {
    let zero = vdupq_n_u8(b'0');
    let nine = vdupq_n_u8(b'9');
    let ascii_a = vdupq_n_u8(b'a');
    let ascii_f = vdupq_n_u8(b'f');
    let case_mask = vdupq_n_u8(0x20);

    let digit_val = vsubq_u8(chars, zero);
    let is_digit = vandq_u8(vcgeq_u8(chars, zero), vcleq_u8(chars, nine));

    let lower = vorrq_u8(chars, case_mask);
    let letter_val = vaddq_u8(vsubq_u8(lower, ascii_a), vdupq_n_u8(10));
    let is_letter = vandq_u8(vcgeq_u8(lower, ascii_a), vcleq_u8(lower, ascii_f));

    let nibble = vbslq_u8(is_digit, digit_val, letter_val);
    let valid = vorrq_u8(is_digit, is_letter);
    (nibble, valid)
}
