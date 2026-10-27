#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unexpected_cfgs)]

use super::generic::{self, Vector};
use crate::{Output, get_chars_table};
use core::mem::MaybeUninit;

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

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
        // `cpufeatures` 0.2 expands to `u8::max_value()`, which recent toolchains
        // deprecate; 0.3 needs a newer MSRV than this crate supports.
        #[allow(deprecated)]
        mod cpuid {
            cpufeatures::new!(cpuid_sse2, "sse2");
            pub(super) use cpuid_sse2::get as has_sse2;
            cpufeatures::new!(cpuid_ssse3, "ssse3");
            pub(super) use cpuid_ssse3::get as has_ssse3;
            cpufeatures::new!(cpuid_avx2, "avx2");
            pub(super) use cpuid_avx2::get as has_avx2;
        }
        use cpuid::{has_avx2, has_sse2, has_ssse3};
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

/// # Safety
///
/// The CPU must support AVX2, and `output` must be at least `2 * input.len()` bytes long.
#[inline(never)]
#[target_feature(enable = "avx2")]
unsafe fn encode_avx2<const UPPER: bool>(input: &[u8], output: impl Output) {
    generic::encode_unaligned_chunks::<UPPER, 2, false, _>(input, output, |chunk| {
        encode_chunk_avx2::<UPPER>(chunk)
    });
}

#[target_feature(enable = "ssse3")]
unsafe fn encode_ssse3<const UPPER: bool>(input: &[u8], output: impl Output) {
    generic::encode_unaligned_chunks::<UPPER, 2, false, _>(input, output, |chunk| {
        encode_chunk_ssse3::<UPPER>(chunk)
    });
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

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn check_avx2(input: &[u8]) -> bool {
    let digit_bias = _mm256_set1_epi8(0xB0_u8 as i8); // '0' + 0x80
    let alpha_bias = _mm256_set1_epi8(0xC1_u8 as i8); // 'A' + 0x80
    let case_mask = _mm256_set1_epi8(0xDF_u8 as i8);
    let digit_threshold = _mm256_set1_epi8(-118); // i8::MIN + 10
    let alpha_threshold = _mm256_set1_epi8(-122); // i8::MIN + 6

    let valid = |chunk: __m256i| {
        let x1 = _mm256_sub_epi8(chunk, digit_bias);
        let m1 = _mm256_cmpgt_epi8(digit_threshold, x1);

        let x2 = _mm256_sub_epi8(_mm256_and_si256(chunk, case_mask), alpha_bias);
        let m2 = _mm256_cmpgt_epi8(alpha_threshold, x2);

        _mm256_or_si256(m1, m2)
    };

    generic::check_unaligned_chunks(
        input,
        valid,
        |a, b| _mm256_and_si256(a, b),
        |mask| _mm256_movemask_epi8(mask) == -1,
    )
}

#[target_feature(enable = "sse2")]
unsafe fn check_sse2(input: &[u8]) -> bool {
    generic::check_unaligned_chunks(
        input,
        |chunk| valid_sse2(chunk),
        |a, b| _mm_and_si128(a, b),
        |mask| _mm_movemask_epi8(mask) == 0xffff,
    )
}

#[inline]
#[target_feature(enable = "sse2")]
unsafe fn valid_sse2(chunk: __m128i) -> __m128i {
    let digit_bias = _mm_set1_epi8(0xB0_u8 as i8);
    let alpha_bias = _mm_set1_epi8(0xC1_u8 as i8);
    let case_mask = _mm_set1_epi8(0xDF_u8 as i8);
    let digit_threshold = _mm_set1_epi8(-118);
    let alpha_threshold = _mm_set1_epi8(-122);

    let x1 = _mm_sub_epi8(chunk, digit_bias);
    let m1 = _mm_cmpgt_epi8(digit_threshold, x1);

    let x2 = _mm_sub_epi8(_mm_and_si128(chunk, case_mask), alpha_bias);
    let m2 = _mm_cmpgt_epi8(alpha_threshold, x2);

    _mm_or_si128(m1, m2)
}

/// Single-pass hex decode with validation.
///
/// # Safety
///
/// `output.len()` must equal `input.len() / 2`, and `input.len()` must be even, because the
/// tails overlap on hex pairs.
#[inline]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    match () {
        _ if has_avx2() => decode_checked_avx2(input, output),
        _ if has_ssse3() => decode_checked_ssse3(input, output),
        _ => generic::decode_checked(input, output),
    }
}

/// # Safety
///
/// The CPU must support AVX2, `output.len()` must equal `input.len() / 2`, and `input.len()` must
/// be even.
#[target_feature(enable = "avx2")]
unsafe fn decode_checked_avx2(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    generic::decode_checked_unaligned_chunks(
        input,
        output,
        |chunk| decode_nibbles_avx2(chunk),
        |a, b| _mm256_or_si256(a, b),
        |nibbles| decode_all_valid_avx2(nibbles),
        |a, b| pack_avx2(a, b),
    )
}

/// # Safety
///
/// The CPU must support SSSE3, `output.len()` must equal `input.len() / 2`, and `input.len()` must
/// be even.
#[target_feature(enable = "ssse3")]
unsafe fn decode_checked_ssse3(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    generic::decode_checked_unaligned_chunks(
        input,
        output,
        |chunk| decode_nibbles_ssse3(chunk),
        |a, b| _mm_or_si128(a, b),
        |nibbles| decode_all_valid_ssse3(nibbles),
        |a, b| pack_ssse3(a, b),
    )
}

/// Defines `#[target_feature]` functions that call the shared kernels of a vector type.
///
/// The kernels are `#[inline(always)]` without target features, so their intrinsics inline only
/// into a function that has the features. Before Rust 1.86 a closure does not inherit the features
/// of the function around it, so the closures passed to the chunk helpers call these functions
/// instead of the kernels.
macro_rules! feature_kernels {
    ($feature:literal, $vector:ty, $encode:ident, $nibbles:ident, $all_valid:ident, $pack:ident) => {
        #[inline]
        #[target_feature(enable = $feature)]
        unsafe fn $encode<const UPPER: bool>(chunk: $vector) -> [$vector; 2] {
            generic::encode_kernel::<UPPER, _>(chunk)
        }

        #[inline]
        #[target_feature(enable = $feature)]
        unsafe fn $nibbles(chunk: $vector) -> $vector {
            generic::decode_kernel(chunk)
        }

        #[inline]
        #[target_feature(enable = $feature)]
        unsafe fn $all_valid(nibbles: $vector) -> bool {
            generic::decode_all_valid(nibbles)
        }

        #[inline]
        #[target_feature(enable = $feature)]
        unsafe fn $pack(a: $vector, b: $vector) -> $vector {
            <$vector>::pack(a, b)
        }
    };
}

feature_kernels!(
    "avx2",
    __m256i,
    encode_chunk_avx2,
    decode_nibbles_avx2,
    decode_all_valid_avx2,
    pack_avx2
);
feature_kernels!(
    "ssse3",
    __m128i,
    encode_chunk_ssse3,
    decode_nibbles_ssse3,
    decode_all_valid_ssse3,
    pack_ssse3
);

generic::impl_vector! {
    __m128i;
    and = _mm_and_si128,
    or = _mm_or_si128,
    sub = _mm_sub_epi8,
    saturating_add = _mm_adds_epu8,
    saturating_sub = _mm_subs_epu8,
    min = _mm_min_epu8;

    #[inline(always)]
    unsafe fn splat(x: u8) -> Self {
        _mm_set1_epi8(x as i8)
    }

    #[inline(always)]
    unsafe fn shr4(self) -> Self {
        _mm_and_si128(_mm_srli_epi16(self, 4), _mm_set1_epi8(0x0f))
    }

    /// Requires SSSE3.
    #[inline(always)]
    unsafe fn hex_digits<const UPPER: bool>(self) -> Self {
        let table = _mm_loadu_si128(get_chars_table::<UPPER>().as_ptr().cast());
        _mm_shuffle_epi8(table, self)
    }

    #[inline(always)]
    unsafe fn interleave(a: Self, b: Self) -> [Self; 2] {
        [_mm_unpacklo_epi8(a, b), _mm_unpackhi_epi8(a, b)]
    }

    /// Requires SSSE3: merges each pair as `hi * 16 + lo` with `maddubs`.
    #[inline(always)]
    unsafe fn pack(a: Self, b: Self) -> Self {
        let weights = _mm_set1_epi16(0x0110);
        _mm_packus_epi16(_mm_maddubs_epi16(a, weights), _mm_maddubs_epi16(b, weights))
    }

    #[inline(always)]
    unsafe fn is_zero(self) -> bool {
        _mm_movemask_epi8(_mm_cmpeq_epi8(self, _mm_setzero_si128())) == 0xffff
    }
}

generic::impl_vector! {
    __m256i;
    and = _mm256_and_si256,
    or = _mm256_or_si256,
    sub = _mm256_sub_epi8,
    saturating_add = _mm256_adds_epu8,
    saturating_sub = _mm256_subs_epu8,
    min = _mm256_min_epu8;

    #[inline(always)]
    unsafe fn splat(x: u8) -> Self {
        _mm256_set1_epi8(x as i8)
    }

    #[inline(always)]
    unsafe fn shr4(self) -> Self {
        _mm256_and_si256(_mm256_srli_epi16(self, 4), _mm256_set1_epi8(0x0f))
    }

    #[inline(always)]
    unsafe fn hex_digits<const UPPER: bool>(self) -> Self {
        let table = _mm_loadu_si128(get_chars_table::<UPPER>().as_ptr().cast());
        _mm256_shuffle_epi8(_mm256_broadcastsi128_si256(table), self)
    }

    /// Unpacking works within each 128-bit lane, so the lanes are put back in order afterwards.
    #[inline(always)]
    unsafe fn interleave(a: Self, b: Self) -> [Self; 2] {
        let lo = _mm256_unpacklo_epi8(a, b);
        let hi = _mm256_unpackhi_epi8(a, b);
        [
            _mm256_permute2x128_si256(lo, hi, 0x20),
            _mm256_permute2x128_si256(lo, hi, 0x31),
        ]
    }

    /// Merges each pair as `hi * 16 + lo` with `maddubs`. Packing works within each 128-bit lane,
    /// so the 64-bit blocks are put back in order afterwards.
    #[inline(always)]
    unsafe fn pack(a: Self, b: Self) -> Self {
        let weights = _mm256_set1_epi16(0x0110);
        let packed = _mm256_packus_epi16(
            _mm256_maddubs_epi16(a, weights),
            _mm256_maddubs_epi16(b, weights),
        );
        _mm256_permute4x64_epi64(packed, 0b11_01_10_00)
    }

    #[inline(always)]
    unsafe fn is_zero(self) -> bool {
        _mm256_testz_si256(self, self) != 0
    }
}
