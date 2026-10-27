#![allow(unsafe_op_in_unsafe_fn)]

use super::generic::{self, Vector};
use crate::{Output, get_chars_table};
use core::mem::MaybeUninit;
use core::simd::prelude::*;

type Simd = u8x16;

pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: impl Output) {
    generic::encode_unaligned_chunks::<UPPER, 2, false, _>(input, output, |chunk: Simd| {
        generic::encode_kernel::<UPPER, _>(chunk)
    });
}

pub(crate) fn check(input: &[u8]) -> bool {
    generic::check_unaligned_chunks(
        input,
        |chunk: Simd| unsafe { generic::check_kernel(chunk) },
        |a, b| unsafe { a.or(b) },
        |invalid| unsafe { invalid.is_zero() },
    )
}

/// Single-pass hex decode with validation.
///
/// # Safety
///
/// `output.len()` must equal `input.len() / 2`, and `input.len()` must be even, because the
/// tails overlap on hex pairs.
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    generic::decode_checked_unaligned_chunks(
        input,
        output,
        |chunk: Simd| generic::decode_kernel(chunk),
        |a, b| a.or(b),
        |nibbles| generic::decode_all_valid(nibbles),
        |a, b| Simd::pack(a, b),
    )
}

generic::impl_vector! {
    Simd;
    and = core::ops::BitAnd::bitand,
    or = core::ops::BitOr::bitor,
    sub = core::ops::Sub::sub,
    saturating_add = core::simd::num::SimdUint::saturating_add,
    saturating_sub = core::simd::num::SimdUint::saturating_sub,
    min = Simd::simd_min;

    #[inline(always)]
    unsafe fn splat(x: u8) -> Self {
        Simd::splat(x)
    }

    #[inline(always)]
    unsafe fn shr4(self) -> Self {
        self >> Simd::splat(4)
    }

    /// `swizzle_dyn` is a `tbl` on aarch64, where `core` is built with NEON. Elsewhere `core` is
    /// built without SSSE3 or simd128, so it compiles to scalar code, and this adds the distance
    /// from `'0'` to the letter to nibbles above 9 instead.
    #[inline(always)]
    unsafe fn hex_digits<const UPPER: bool>(self) -> Self {
        if cfg!(any(target_arch = "aarch64", target_arch = "arm64ec")) {
            Simd::from_array(*get_chars_table::<UPPER>()).swizzle_dyn(self)
        } else {
            let letter = if UPPER { b'A' } else { b'a' } - 10 - b'0';
            let adjust = self
                .simd_gt(Simd::splat(9))
                .select(Simd::splat(letter), Simd::splat(0));
            self + Simd::splat(b'0') + adjust
        }
    }

    #[inline(always)]
    unsafe fn interleave(a: Self, b: Self) -> [Self; 2] {
        let (lo, hi) = Simd::interleave(a, b);
        [lo, hi]
    }

    #[inline(always)]
    unsafe fn pack(a: Self, b: Self) -> Self {
        let (hi, lo) = Simd::deinterleave(a, b);
        // The nibbles are at most 15, so no bit of the 16-bit shift crosses into the other byte,
        // whatever the byte order. Unlike an 8-bit shift, it needs no mask on x86.
        let hi = core::mem::transmute::<u16x8, Simd>(
            core::mem::transmute::<Simd, u16x8>(hi) << u16x8::splat(4),
        );
        hi | lo
    }

    #[inline(always)]
    unsafe fn is_zero(self) -> bool {
        self.simd_eq(Simd::splat(0)).all()
    }
}
