#![allow(unsafe_op_in_unsafe_fn)]

use super::generic::{self, Vector};
use crate::{Output, get_chars_table};
use core::arch::aarch64::*;
use core::mem::MaybeUninit;

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
    // Finishing in the loop keeps the final step off the hot path for multiples of the chunk size.
    generic::encode_unaligned_chunks::<UPPER, 1, true, _>(input, output, |chunk: uint8x16_t| {
        generic::encode_kernel::<UPPER, _>(chunk)
    });
}

#[inline]
pub(crate) fn check(input: &[u8]) -> bool {
    if !has_neon() {
        return generic::check(input);
    }
    unsafe { check_neon(input) }
}

/// `NIB[i]` is `0x10 | nibble` if `b'0' + i` is a hex digit, else 0.
const NIB: [u8; 64] = {
    let mut table = [0; 64];
    let mut i = 0;
    while i < 64 {
        let nibble = crate::HEX_DECODE_LUT[b'0' as usize + i];
        if nibble != crate::NIL {
            table[i] = 0x10 | nibble;
        }
        i += 1;
    }
    table
};

/// Converts ASCII hex to `0x10 | nibble`; any other byte becomes 0.
///
/// A 64-entry table lookup replaces the arithmetic of the generic kernels: `vqtbl4q_u8` gives 0
/// for indices past the table, so bytes outside `'0'..='o'` map to 0.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn nibbles(chunk: uint8x16_t) -> uint8x16_t {
    vqtbl4q_u8(vld1q_u8_x4(NIB.as_ptr()), vsubq_u8(chunk, vdupq_n_u8(b'0')))
}

#[target_feature(enable = "neon")]
pub(crate) unsafe fn check_neon(input: &[u8]) -> bool {
    generic::check_unaligned_chunks(
        input,
        |chunk| nibbles(chunk),
        |a, b| vminq_u8(a, b),
        // A zero byte marks an invalid character.
        |n| vminvq_u8(n) != 0,
    )
}

/// Single-pass hex decode with validation.
///
/// Characters are looked up in a table that gives `0x10 | nibble` for hex digits and 0 otherwise,
/// so a zero byte marks an invalid character. Pairs are merged with `vsliq_n_u8`, which drops the
/// marker bit.
///
/// # Safety
///
/// `output.len()` must equal `input.len() / 2`, and `input.len()` must be even, because the
/// tails overlap on hex pairs.
#[inline]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    if cfg!(miri) || !has_neon() {
        return generic::decode_checked(input, output);
    }
    decode_checked_neon(input, output)
}

#[target_feature(enable = "neon")]
unsafe fn decode_checked_neon(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    generic::decode_checked_unaligned_chunks(
        input,
        output,
        |chunk| nibbles(chunk),
        |a, b| vminq_u8(a, b),
        |n| vminvq_u8(n) != 0,
        |a, b| uint8x16_t::pack(a, b),
    )
}

generic::impl_vector! {
    uint8x16_t;
    and = vandq_u8,
    or = vorrq_u8,
    sub = vsubq_u8,
    saturating_add = vqaddq_u8,
    saturating_sub = vqsubq_u8,
    min = vminq_u8;

    #[inline(always)]
    unsafe fn splat(x: u8) -> Self {
        vdupq_n_u8(x)
    }

    #[inline(always)]
    unsafe fn shr4(self) -> Self {
        vshrq_n_u8::<4>(self)
    }

    #[inline(always)]
    unsafe fn hex_digits<const UPPER: bool>(self) -> Self {
        vqtbl1q_u8(vld1q_u8(get_chars_table::<UPPER>().as_ptr()), self)
    }

    #[inline(always)]
    unsafe fn interleave(a: Self, b: Self) -> [Self; 2] {
        let zipped = vzipq_u8(a, b);
        [zipped.0, zipped.1]
    }

    /// Also accepts bytes above 15: `vsliq_n_u8::<4>` shifts the high nibbles left by four and keeps
    /// only the low four bits of each low nibble, so the marker bit of `nibbles` drops out, as
    /// [`decode_checked_neon`] needs.
    #[inline(always)]
    unsafe fn pack(a: Self, b: Self) -> Self {
        vsliq_n_u8::<4>(vuzp2q_u8(a, b), vuzp1q_u8(a, b))
    }

    #[inline(always)]
    unsafe fn is_zero(self) -> bool {
        vmaxvq_u8(self) == 0
    }
}
