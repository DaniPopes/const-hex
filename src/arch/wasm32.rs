#![allow(unsafe_op_in_unsafe_fn)]

use super::generic::{self, Vector};
use crate::{Output, get_chars_table};
use core::arch::wasm32::*;
use core::mem::MaybeUninit;

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: impl Output) {
    generic::encode_unaligned_chunks::<UPPER, 1, _, _>(input, output, |chunk: v128| {
        generic::encode_kernel::<UPPER, _>(chunk)
    });
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) fn check(input: &[u8]) -> bool {
    generic::check_unaligned_chunks(
        input,
        |chunk: v128| unsafe { generic::check_kernel(chunk) },
        |a, b| unsafe { a.or(b) },
        |invalid| unsafe { invalid.is_zero() },
    )
}

/// Single-pass hex decode with validation.
#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    generic::decode_checked_unaligned_chunks(
        input,
        output,
        |chunk: v128| generic::decode_kernel(chunk),
        |a, b| a.or(b),
        |nibbles| generic::decode_all_valid(nibbles),
        |a, b| v128::pack(a, b),
    )
}

generic::impl_vector! {
    v128;
    and = v128_and,
    or = v128_or,
    sub = u8x16_sub,
    adds = u8x16_add_sat,
    subs = u8x16_sub_sat,
    min = u8x16_min;

    #[inline(always)]
    unsafe fn splat(x: u8) -> Self {
        u8x16_splat(x)
    }

    #[inline(always)]
    unsafe fn shr4(self) -> Self {
        u8x16_shr(self, 4)
    }

    #[inline(always)]
    unsafe fn hex_digits<const UPPER: bool>(self) -> Self {
        u8x16_swizzle(v128_load(get_chars_table::<UPPER>().as_ptr().cast()), self)
    }

    #[inline(always)]
    unsafe fn zip(a: Self, b: Self) -> [Self; 2] {
        #[rustfmt::skip]
        let lo = u8x16_shuffle::<0, 16, 1, 17, 2, 18, 3, 19, 4, 20, 5, 21, 6, 22, 7, 23>(a, b);
        #[rustfmt::skip]
        let hi = u8x16_shuffle::<8, 24, 9, 25, 10, 26, 11, 27, 12, 28, 13, 29, 14, 30, 15, 31>(a, b);
        [lo, hi]
    }

    #[inline(always)]
    unsafe fn pack(a: Self, b: Self) -> Self {
        #[rustfmt::skip]
        let hi = u8x16_shuffle::<0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30>(a, b);
        #[rustfmt::skip]
        let lo = u8x16_shuffle::<1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31>(a, b);
        // The nibbles are at most 15, so the 16-bit shift cannot carry into the next byte.
        v128_or(i16x8_shl(hi, 4), lo)
    }

    #[inline(always)]
    unsafe fn is_zero(self) -> bool {
        !v128_any_true(self)
    }
}
