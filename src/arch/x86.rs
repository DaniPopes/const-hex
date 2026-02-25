#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unexpected_cfgs)]

use super::generic;
use crate::{get_chars_table, Output};

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

pub(crate) const USE_CHECK_FN: bool = true;

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        #[inline(always)]
        fn has_sse2() -> bool {
            std::arch::is_x86_feature_detected!("sse2")
        }
        #[inline(always)]
        fn has_avx2() -> bool {
            std::arch::is_x86_feature_detected!("avx2")
        }
    } else {
        cpufeatures::new!(cpuid_sse2, "sse2");
        use cpuid_sse2::get as has_sse2;
        cpufeatures::new!(cpuid_avx2, "avx2");
        use cpuid_avx2::get as has_avx2;
    }
}

// AVX2 versions modified from [`faster-hex`](https://github.com/nervosnetwork/faster-hex/blob/856aba7b141a5fe16113fae110d535065882f25a/src/decode.rs),
// themselves taken from [`zbjornson/fast-hex`](https://github.com/zbjornson/fast-hex/blob/a3487bca95127634a61bfeae8f8bfc8f0e5baa3f/src/hex.cc).

#[inline]
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: impl Output) {
    if !has_avx2() {
        return generic::encode::<UPPER>(input, output);
    }
    encode_avx2::<UPPER>(input, output);
}

/// AVX2 encode processing 32 input bytes → 64 output bytes per iteration.
#[inline(never)]
#[target_feature(enable = "avx2")]
unsafe fn encode_avx2<const UPPER: bool>(input: &[u8], output: impl Output) {
    // Process 32 bytes at a time (produces 64 bytes output)
    generic::encode_unaligned_chunks::<UPPER, _, _>(input, output, |av: __m256i| {
        encode_bytes32::<UPPER>(av)
    });
}

/// Encode 32 bytes into 64 hex characters (two __m256i).
///
/// Algorithm: split each byte into high/low nibbles, interleave them,
/// then use LUT lookup to convert nibbles to hex characters.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn encode_bytes32<const UPPER: bool>(input: __m256i) -> [__m256i; 2] {
    let lut =
        _mm256_broadcastsi128_si256(_mm_lddqu_si128(get_chars_table::<UPPER>().as_ptr().cast()));
    let mask_lo = _mm256_set1_epi8(0x0f);

    // Split into high and low nibbles
    let hi = _mm256_and_si256(_mm256_srli_epi16(input, 4), mask_lo);
    let lo = _mm256_and_si256(input, mask_lo);

    // Interleave high and low nibbles: [h0,l0,h1,l1,...] for each 128-bit lane
    let mixed_lo = _mm256_unpacklo_epi8(hi, lo); // bytes 0-7 of each lane
    let mixed_hi = _mm256_unpackhi_epi8(hi, lo); // bytes 8-15 of each lane

    // The unpack operates within 128-bit lanes, so we need to fix the order:
    // mixed_lo has: [lane0 bytes 0-7 interleaved, lane1 bytes 0-7 interleaved]
    // mixed_hi has: [lane0 bytes 8-15 interleaved, lane1 bytes 8-15 interleaved]
    // We want: first 32 output bytes, then second 32 output bytes

    // Permute to get correct output order:
    // out1 = [lane0 of mixed_lo, lane0 of mixed_hi] = first 16 input bytes encoded
    // out2 = [lane1 of mixed_lo, lane1 of mixed_hi] = second 16 input bytes encoded
    let out1 = _mm256_permute2x128_si256(mixed_lo, mixed_hi, 0x20); // lanes 0,0
    let out2 = _mm256_permute2x128_si256(mixed_lo, mixed_hi, 0x31); // lanes 1,1

    // LUT lookup to convert nibbles to hex characters
    [_mm256_shuffle_epi8(lut, out1), _mm256_shuffle_epi8(lut, out2)]
}

#[inline]
pub(crate) fn check(input: &[u8]) -> bool {
    if !has_sse2() {
        return generic::check(input);
    }
    // Use AVX2 with signed overflow trick for larger inputs (≥128 bytes)
    // Threshold chosen to avoid has_avx2() check overhead for common small sizes (addresses, hashes)
    if input.len() >= 128 && has_avx2() {
        return unsafe { check_avx2(input) };
    }
    // Use original 6-comparison SSE2 algorithm (fast for small inputs like addresses/hashes)
    unsafe { check_sse2(input) }
}

/// Original faster-hex style check - faster for small inputs.
///
/// Modified from [`faster-hex`](https://github.com/nervosnetwork/faster-hex).
#[target_feature(enable = "sse2")]
unsafe fn check_sse2(input: &[u8]) -> bool {
    let ascii_zero = _mm_set1_epi8((b'0' - 1) as i8);
    let ascii_nine = _mm_set1_epi8((b'9' + 1) as i8);
    let ascii_ua = _mm_set1_epi8((b'A' - 1) as i8);
    let ascii_uf = _mm_set1_epi8((b'F' + 1) as i8);
    let ascii_la = _mm_set1_epi8((b'a' - 1) as i8);
    let ascii_lf = _mm_set1_epi8((b'f' + 1) as i8);

    generic::check_unaligned_chunks(input, |chunk: __m128i| {
        let ge0 = _mm_cmpgt_epi8(chunk, ascii_zero);
        let le9 = _mm_cmplt_epi8(chunk, ascii_nine);
        let valid_digit = _mm_and_si128(ge0, le9);

        let geua = _mm_cmpgt_epi8(chunk, ascii_ua);
        let leuf = _mm_cmplt_epi8(chunk, ascii_uf);
        let valid_upper = _mm_and_si128(geua, leuf);

        let gela = _mm_cmpgt_epi8(chunk, ascii_la);
        let lelf = _mm_cmplt_epi8(chunk, ascii_lf);
        let valid_lower = _mm_and_si128(gela, lelf);

        let valid_letter = _mm_or_si128(valid_lower, valid_upper);
        let valid_mask = _mm_movemask_epi8(_mm_or_si128(valid_digit, valid_letter));
        valid_mask == 0xffff
    })
}

/// AVX2 check using signed overflow trick - faster for large inputs.
///
/// Based on "Parsing hex numbers with validation" by Muła & Langdale (Algorithm #3):
/// <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
#[target_feature(enable = "avx2")]
unsafe fn check_avx2(input: &[u8]) -> bool {
    let digit_offset = _mm256_set1_epi8((0x30_u8.wrapping_add(0x80)) as i8);
    let letter_offset = _mm256_set1_epi8((0x41_u8.wrapping_add(0x80)) as i8);
    let case_mask = _mm256_set1_epi8(0xDF_u8 as i8);
    let digit_threshold = _mm256_set1_epi8(-118);
    let letter_threshold = _mm256_set1_epi8(-122);

    generic::check_unaligned_chunks(input, |chunk: __m256i| {
        // Check digits: '0'-'9' (0x30-0x39)
        let x1 = _mm256_sub_epi8(chunk, digit_offset);
        let m1 = _mm256_cmpgt_epi8(digit_threshold, x1);

        // Check letters: 'A'-'F' / 'a'-'f' (normalize case with 0xDF mask)
        let normalized = _mm256_and_si256(chunk, case_mask);
        let x2 = _mm256_sub_epi8(normalized, letter_offset);
        let m2 = _mm256_cmpgt_epi8(letter_threshold, x2);

        // All bytes must match either digit or letter pattern
        let valid = _mm256_or_si256(m1, m2);
        _mm256_movemask_epi8(valid) == -1 // 0xffffffff as i32
    })
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

// Not used.
pub(crate) use generic::decode_checked;
