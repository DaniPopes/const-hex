use super::generic;
use core::arch::wasm32::*;

pub(crate) const USE_CHECK_FN: bool = false;

pub(crate) use generic::{decode_checked, decode_unchecked, encode};

#[inline(always)]
fn is_available() -> bool {
    cfg!(target_feature = "simd128")
}

#[inline]
pub(crate) fn check(input: &[u8]) -> bool {
    if !is_available() {
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
