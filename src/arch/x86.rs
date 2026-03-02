#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unexpected_cfgs)]

use super::generic;
use crate::{get_chars_table, Output};

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

pub(crate) const USE_CHECK_FN: bool = false;

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        #[inline(always)]
        fn has_sse2() -> bool {
            std::arch::is_x86_feature_detected!("sse2")
        }
        #[inline(always)]
        fn has_ssse3() -> bool {
            std::arch::is_x86_feature_detected!("ssse3")
        }
        #[inline(always)]
        fn has_avx2() -> bool {
            std::arch::is_x86_feature_detected!("avx2")
        }
    } else {
        cpufeatures::new!(cpuid_sse2, "sse2");
        use cpuid_sse2::get as has_sse2;
        cpufeatures::new!(cpuid_ssse3, "ssse3");
        use cpuid_ssse3::get as has_ssse3;
        cpufeatures::new!(cpuid_avx2, "avx2");
        use cpuid_avx2::get as has_avx2;
    }
}

// Decode modified from [`faster-hex`](https://github.com/nervosnetwork/faster-hex/blob/856aba7b141a5fe16113fae110d535065882f25a/src/decode.rs),
// itself taken from [`zbjornson/fast-hex`](https://github.com/zbjornson/fast-hex/blob/a3487bca95127634a61bfeae8f8bfc8f0e5baa3f/src/hex.cc).

#[inline]
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: impl Output) {
    match () {
        _ if has_avx2() => encode_avx2::<UPPER>(input, output),
        _ if has_ssse3() => encode_ssse3::<UPPER>(input, output),
        _ => generic::encode::<UPPER>(input, output),
    }
}

#[inline(never)]
#[target_feature(enable = "avx2")]
unsafe fn encode_avx2<const UPPER: bool>(input: &[u8], output: impl Output) {
    generic::encode_unaligned_chunks_with::<UPPER, _, _, _>(
        input,
        output,
        |av| encode_chunk_avx2::<UPPER>(av),
        |remainder, out| {
            generic::encode_one_unaligned_chunk::<UPPER, _, _>(remainder, out, |av| {
                encode_chunk_ssse3::<UPPER>(av)
            })
        },
    );
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn encode_chunk_avx2<const UPPER: bool>(input: __m256i) -> [__m256i; 2] {
    let lut =
        _mm256_broadcastsi128_si256(_mm_lddqu_si128(get_chars_table::<UPPER>().as_ptr().cast()));
    let mask_lo = _mm256_set1_epi8(0x0f);

    let hi = _mm256_and_si256(_mm256_srli_epi16(input, 4), mask_lo);
    let lo = _mm256_and_si256(input, mask_lo);

    let mixed_lo = _mm256_unpacklo_epi8(hi, lo);
    let mixed_hi = _mm256_unpackhi_epi8(hi, lo);

    let out1 = _mm256_permute2x128_si256(mixed_lo, mixed_hi, 0x20);
    let out2 = _mm256_permute2x128_si256(mixed_lo, mixed_hi, 0x31);

    [
        _mm256_shuffle_epi8(lut, out1),
        _mm256_shuffle_epi8(lut, out2),
    ]
}

#[target_feature(enable = "ssse3")]
unsafe fn encode_ssse3<const UPPER: bool>(input: &[u8], output: impl Output) {
    generic::encode_unaligned_chunks::<UPPER, _, _>(input, output, |av| {
        encode_chunk_ssse3::<UPPER>(av)
    });
}

#[inline]
#[target_feature(enable = "ssse3")]
unsafe fn encode_chunk_ssse3<const UPPER: bool>(input: __m128i) -> [__m128i; 2] {
    let lut = _mm_lddqu_si128(get_chars_table::<UPPER>().as_ptr().cast());
    let mask_lo = _mm_set1_epi8(0x0f);

    let hi = _mm_and_si128(_mm_srli_epi16(input, 4), mask_lo);
    let lo = _mm_and_si128(input, mask_lo);

    let out1 = _mm_unpacklo_epi8(hi, lo);
    let out2 = _mm_unpackhi_epi8(hi, lo);

    [_mm_shuffle_epi8(lut, out1), _mm_shuffle_epi8(lut, out2)]
}

/// Hex check using signed overflow trick: bias each valid range so it starts at `i8::MIN`,
/// then a single `cmpgt(start + len, x)` checks if `x` falls within the range.
///
/// - Digits '0'..'9' (0x30..0x39): bias by 0xB0 maps to -128..-119, threshold -118 (10 values).
/// - Letters 'A'..'F' (0x41..0x46): bias by 0xC1 maps to -128..-123, threshold -122 (6 values).
///   Case folded with 0xDF mask so 'a'..'f' is handled identically.
///
/// Based on Muła & Langdale:
/// <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
#[inline]
pub(crate) fn check(input: &[u8]) -> bool {
    match () {
        _ if has_avx2() => unsafe { check_avx2(input) },
        _ if has_sse2() => unsafe { check_sse2(input) },
        _ => generic::check(input),
    }
}

#[target_feature(enable = "avx2")]
unsafe fn check_avx2(input: &[u8]) -> bool {
    let digit_bias = _mm256_set1_epi8(0xB0_u8 as i8); // '0' + 0x80
    let alpha_bias = _mm256_set1_epi8(0xC1_u8 as i8); // 'A' + 0x80
    let case_mask = _mm256_set1_epi8(0xDF_u8 as i8);
    let digit_threshold = _mm256_set1_epi8(-118); // i8::MIN + 10
    let alpha_threshold = _mm256_set1_epi8(-122); // i8::MIN + 6

    generic::check_unaligned_chunks_with(
        input,
        |chunk| {
            let x1 = _mm256_sub_epi8(chunk, digit_bias);
            let m1 = _mm256_cmpgt_epi8(digit_threshold, x1);

            let x2 = _mm256_sub_epi8(_mm256_and_si256(chunk, case_mask), alpha_bias);
            let m2 = _mm256_cmpgt_epi8(alpha_threshold, x2);

            _mm256_movemask_epi8(_mm256_or_si256(m1, m2)) == -1
        },
        |remainder| generic::check_one_unaligned_chunk(remainder, |c| check_chunk_sse2(c)),
    )
}

#[target_feature(enable = "sse2")]
unsafe fn check_sse2(input: &[u8]) -> bool {
    generic::check_unaligned_chunks(input, |c| check_chunk_sse2(c))
}

#[inline]
#[target_feature(enable = "sse2")]
unsafe fn check_chunk_sse2(chunk: __m128i) -> bool {
    let digit_bias = _mm_set1_epi8(0xB0_u8 as i8);
    let alpha_bias = _mm_set1_epi8(0xC1_u8 as i8);
    let case_mask = _mm_set1_epi8(0xDF_u8 as i8);
    let digit_threshold = _mm_set1_epi8(-118);
    let alpha_threshold = _mm_set1_epi8(-122);

    let x1 = _mm_sub_epi8(chunk, digit_bias);
    let m1 = _mm_cmpgt_epi8(digit_threshold, x1);

    let x2 = _mm_sub_epi8(_mm_and_si128(chunk, case_mask), alpha_bias);
    let m2 = _mm_cmpgt_epi8(alpha_threshold, x2);

    _mm_movemask_epi8(_mm_or_si128(m1, m2)) == 0xffff
}

#[inline]
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: &mut [u8]) {
    if !has_avx2() {
        return generic::decode_unchecked(input, output);
    }
    decode_avx2(input, output);
}

#[target_feature(enable = "avx2")]
unsafe fn decode_avx2(input: &[u8], output: &mut [u8]) {
    #[rustfmt::skip]
    let mask_a = _mm256_setr_epi8(
        0, -1, 2, -1, 4, -1, 6, -1, 8, -1, 10, -1, 12, -1, 14, -1,
        0, -1, 2, -1, 4, -1, 6, -1, 8, -1, 10, -1, 12, -1, 14, -1,
    );
    #[rustfmt::skip]
    let mask_b = _mm256_setr_epi8(
        1, -1, 3, -1, 5, -1, 7, -1, 9, -1, 11, -1, 13, -1, 15, -1,
        1, -1, 3, -1, 5, -1, 7, -1, 9, -1, 11, -1, 13, -1, 15, -1
    );
    generic::decode_unchecked_unaligned_chunks(input, output, |[av1, av2]: [__m256i; 2]| {
        let mut a1 = _mm256_shuffle_epi8(av1, mask_a);
        let mut b1 = _mm256_shuffle_epi8(av1, mask_b);
        let mut a2 = _mm256_shuffle_epi8(av2, mask_a);
        let mut b2 = _mm256_shuffle_epi8(av2, mask_b);

        a1 = unhex(a1);
        a2 = unhex(a2);
        b1 = unhex(b1);
        b2 = unhex(b2);

        nib2byte(a1, b1, a2, b2)
    });
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn unhex(value: __m256i) -> __m256i {
    let sr6 = _mm256_srai_epi16(value, 6);
    let and15 = _mm256_and_si256(value, _mm256_set1_epi16(0xf));
    let mul = _mm256_maddubs_epi16(sr6, _mm256_set1_epi16(9));
    _mm256_add_epi16(mul, and15)
}

// (a << 4) | b;
// a and b must be 16-bit elements. Output is packed 8-bit elements.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn nib2byte(a1: __m256i, b1: __m256i, a2: __m256i, b2: __m256i) -> __m256i {
    let a4_1 = _mm256_slli_epi16(a1, 4);
    let a4_2 = _mm256_slli_epi16(a2, 4);
    let a4orb_1 = _mm256_or_si256(a4_1, b1);
    let a4orb_2 = _mm256_or_si256(a4_2, b2);
    let pck1 = _mm256_packus_epi16(a4orb_1, a4orb_2); // lo1 lo2 hi1 hi2
    const _0213: i32 = 0b11_01_10_00;
    _mm256_permute4x64_epi64(pck1, _0213)
}

/// Single-pass hex decode with validation using Muła & Langdale's Algorithm #3.
///
/// Converts ASCII hex to nibble values and validates simultaneously:
/// - Digits '0'..'9' → 0..9, letters 'A'..'F'/'a'..'f' → 10..15 via saturation arithmetic.
/// - Invalid bytes produce values > 15, detected via `adds_epu8(nibble, 112)` setting the MSB.
/// - Nibble pairs are merged with `maddubs(nibbles, 0x0110)` (hi*16 + lo).
///
/// Based on: <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
#[inline]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [u8]) -> bool {
    if has_avx2() {
        return decode_checked_avx2(input, output);
    }
    generic::decode_checked(input, output)
}

#[target_feature(enable = "avx2")]
unsafe fn decode_checked_avx2(input: &[u8], output: &mut [u8]) -> bool {
    debug_assert_eq!(output.len(), input.len() / 2);

    let add_c6 = _mm256_set1_epi8(0xC6u8 as i8); // 0xFF - b'9'
    let six = _mm256_set1_epi8(6);
    let f0 = _mm256_set1_epi8(0xF0u8 as i8);
    let df = _mm256_set1_epi8(0xDFu8 as i8);
    let big_a = _mm256_set1_epi8(b'A' as i8);
    let ten = _mm256_set1_epi8(10);
    let check_bias = _mm256_set1_epi8(112); // 127 - 15
    let weights = _mm256_set1_epi16(0x0110);

    let in_ptr = input.as_ptr();
    let out_ptr = output.as_mut_ptr();
    let mut i = 0;

    while i + 64 <= input.len() {
        let v1 = _mm256_loadu_si256(in_ptr.add(i).cast());
        let v2 = _mm256_loadu_si256(in_ptr.add(i + 32).cast());

        // Digits '0'..'9' → 0..9, others > 15.
        let d1 = _mm256_sub_epi8(_mm256_subs_epu8(_mm256_add_epi8(v1, add_c6), six), f0);
        let d2 = _mm256_sub_epi8(_mm256_subs_epu8(_mm256_add_epi8(v2, add_c6), six), f0);

        // Letters 'A'..'F'/'a'..'f' → 10..15, others > 15.
        let a1 = _mm256_adds_epu8(_mm256_sub_epi8(_mm256_and_si256(v1, df), big_a), ten);
        let a2 = _mm256_adds_epu8(_mm256_sub_epi8(_mm256_and_si256(v2, df), big_a), ten);

        // Valid nibble wins (0..15), invalid stays > 15.
        let n1 = _mm256_min_epu8(d1, a1);
        let n2 = _mm256_min_epu8(d2, a2);

        // Validate: saturating add sets MSB if nibble > 15.
        let c1 = _mm256_adds_epu8(n1, check_bias);
        let c2 = _mm256_adds_epu8(n2, check_bias);
        if _mm256_movemask_epi8(_mm256_or_si256(c1, c2)) != 0 {
            return false;
        }

        // Merge nibble pairs: hi * 16 + lo.
        let b1 = _mm256_maddubs_epi16(n1, weights);
        let b2 = _mm256_maddubs_epi16(n2, weights);
        let packed = _mm256_packus_epi16(b1, b2);
        let result = _mm256_permute4x64_epi64(packed, 0b11_01_10_00);

        _mm256_storeu_si256(out_ptr.add(i / 2).cast(), result);
        i += 64;
    }

    // 32-byte remainder (one __m256i = 32 hex bytes → 16 output bytes).
    if i + 32 <= input.len() {
        let v = _mm256_loadu_si256(in_ptr.add(i).cast());

        let d = _mm256_sub_epi8(_mm256_subs_epu8(_mm256_add_epi8(v, add_c6), six), f0);
        let a = _mm256_adds_epu8(_mm256_sub_epi8(_mm256_and_si256(v, df), big_a), ten);
        let n = _mm256_min_epu8(d, a);

        if _mm256_movemask_epi8(_mm256_adds_epu8(n, check_bias)) != 0 {
            return false;
        }

        let merged = _mm256_maddubs_epi16(n, weights);
        let packed = _mm256_packus_epi16(merged, _mm256_setzero_si256());
        let result = _mm256_permute4x64_epi64(packed, 0b11_01_10_00);

        // Store lower 16 bytes.
        _mm_storeu_si128(out_ptr.add(i / 2).cast(), _mm256_castsi256_si128(result));
        i += 32;
    }

    if i < input.len() {
        generic::decode_checked(&input[i..], &mut output[i / 2..])
    } else {
        true
    }
}
