#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unexpected_cfgs)]

use super::generic;
use crate::{Output, get_chars_table};
use core::mem::MaybeUninit;

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

    let len = input.len();
    let ptr = input.as_ptr();

    if len < 4 {
        return generic::check(input);
    }

    let mut tail = input;
    if len >= 128 {
        let mut chunks = input.chunks_exact(128);
        for chunk in &mut chunks {
            let [c0, c1, c2, c3] = chunk.as_ptr().cast::<[__m256i; 4]>().read_unaligned();
            let v01 = _mm256_and_si256(valid(c0), valid(c1));
            let v23 = _mm256_and_si256(valid(c2), valid(c3));
            if _mm256_movemask_epi8(_mm256_and_si256(v01, v23)) != -1 {
                return false;
            }
        }
        tail = chunks.remainder();
    }

    let check32 = |chunk: __m256i| _mm256_movemask_epi8(valid(chunk)) == -1;
    let mut chunks = tail.chunks_exact(32);
    for chunk in &mut chunks {
        if !check32(chunk.as_ptr().cast::<__m256i>().read_unaligned()) {
            return false;
        }
    }
    if chunks.remainder().is_empty() {
        return true;
    }

    // Re-check the last chunk, overlapping what was already checked.
    if len >= 32 {
        check32(ptr.add(len - 32).cast::<__m256i>().read_unaligned())
    } else if len >= 16 {
        check32(_mm256_loadu2_m128i(ptr.add(len - 16).cast(), ptr.cast()))
    } else if len >= 8 {
        let first = _mm_loadl_epi64(ptr.cast());
        let last = _mm_loadl_epi64(ptr.add(len - 8).cast());
        _mm_movemask_epi8(valid_sse2(_mm_unpacklo_epi64(first, last))) == 0xffff
    } else {
        let first = _mm_cvtsi32_si128(ptr.cast::<i32>().read_unaligned());
        let last = _mm_cvtsi32_si128(ptr.add(len - 4).cast::<i32>().read_unaligned());
        _mm_movemask_epi8(valid_sse2(_mm_unpacklo_epi32(first, last))) & 0xff == 0xff
    }
}

#[target_feature(enable = "sse2")]
unsafe fn check_sse2(input: &[u8]) -> bool {
    generic::check_unaligned_chunks(input, |c| _mm_movemask_epi8(valid_sse2(c)) == 0xffff)
}

/// Per-byte validity mask: `0xFF` for hex digits, `0x00` otherwise.
///
/// # Safety
///
/// The CPU must support SSE2.
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

#[inline]
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: &mut [MaybeUninit<u8>]) {
    if !has_avx2() {
        return generic::decode_unchecked(input, output);
    }
    decode_avx2(input, output);
}

#[target_feature(enable = "avx2")]
unsafe fn decode_avx2(input: &[u8], output: &mut [MaybeUninit<u8>]) {
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
/// - Invalid bytes produce values > 15, detected by `testz(nibble, 0xF0)`.
/// - Nibble pairs are merged with `maddubs(nibbles, 0x0110)` (hi*16 + lo).
///
/// Based on: <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
#[inline]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    if has_avx2() {
        return decode_checked_avx2(input, output);
    }
    generic::decode_checked(input, output)
}

#[target_feature(enable = "avx2")]
unsafe fn decode_checked_avx2(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    debug_assert_eq!(output.len(), input.len() / 2);
    debug_assert_eq!(input.len() % 2, 0);

    let len = input.len();
    let ip = input.as_ptr();
    let op = output.as_mut_ptr().cast::<u8>();

    if len < 8 {
        return generic::decode_checked(input, output);
    }

    let d = Decoder::new();

    // Every branch decodes the last chunk last, overlapping what was already decoded.
    // `len` is even, so the overlapping chunk starts on a hex pair and rewrites the same bytes.
    if len >= 64 {
        for i in 0..len / 128 {
            if !d.decode_128(ip.add(i * 128), op.add(i * 64)) {
                return false;
            }
        }
        // At most one 64-character chunk, then a single chunk ending at `len`.
        let mut remainder = len % 128;
        if remainder >= 64 {
            if !d.decode_64(ip.add(len - remainder), op.add((len - remainder) / 2)) {
                return false;
            }
            remainder -= 64;
        }
        match remainder {
            0 => true,
            1..=32 => d.decode_16_32(ip.add(len - 32), 32, op.add((len - 32) / 2)),
            _ => d.decode_64(ip.add(len - 64), op.add((len - 64) / 2)),
        }
    } else if len >= 32 {
        d.decode_16_32(ip, 32, op)
            && (len == 32 || d.decode_16_32(ip.add(len - 32), 32, op.add((len - 32) / 2)))
    } else if len >= 16 {
        d.decode_16_32(ip, len, op)
    } else {
        d.decode_8_15(ip, len, op)
    }
}

/// The vector constants shared by the decode steps, built once per call.
struct Decoder {
    add_c6: __m256i,
    six: __m256i,
    f0: __m256i,
    df: __m256i,
    big_a: __m256i,
    ten: __m256i,
    weights: __m256i,
}

impl Decoder {
    /// # Safety
    ///
    /// The CPU must support AVX2.
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn new() -> Self {
        Self {
            add_c6: _mm256_set1_epi8(0xC6u8 as i8), // 0xFF - b'9'
            six: _mm256_set1_epi8(6),
            f0: _mm256_set1_epi8(0xF0u8 as i8),
            df: _mm256_set1_epi8(0xDFu8 as i8),
            big_a: _mm256_set1_epi8(b'A' as i8),
            ten: _mm256_set1_epi8(10),
            weights: _mm256_set1_epi16(0x0110),
        }
    }

    /// Converts ASCII hex to nibbles: hex digits become 0..15, anything else becomes > 15.
    ///
    /// # Safety
    ///
    /// The CPU must support AVX2.
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn nibbles(&self, v: __m256i) -> __m256i {
        // Digits '0'..'9' → 0..9, others > 15.
        let d = _mm256_sub_epi8(
            _mm256_subs_epu8(_mm256_add_epi8(v, self.add_c6), self.six),
            self.f0,
        );

        // Letters 'A'..'F'/'a'..'f' → 10..15, others > 15.
        let a = _mm256_adds_epu8(
            _mm256_sub_epi8(_mm256_and_si256(v, self.df), self.big_a),
            self.ten,
        );

        // Valid nibble wins (0..15), invalid stays > 15.
        _mm256_min_epu8(d, a)
    }

    /// Merges nibble pairs (hi * 16 + lo) of two vectors and stores the 32 resulting bytes.
    ///
    /// # Safety
    ///
    /// `op` must be valid for writing 32 bytes.
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn store_pair(&self, n1: __m256i, n2: __m256i, op: *mut u8) {
        let b1 = _mm256_maddubs_epi16(n1, self.weights);
        let b2 = _mm256_maddubs_epi16(n2, self.weights);
        let packed = _mm256_packus_epi16(b1, b2);
        _mm256_storeu_si256(op.cast(), _mm256_permute4x64_epi64(packed, 0b11_01_10_00));
    }

    /// Decodes 128 hex characters to 64 bytes. Returns `false` if any is invalid.
    ///
    /// # Safety
    ///
    /// `ip` must be valid for reading 128 bytes and `op` for writing 64.
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn decode_128(&self, ip: *const u8, op: *mut u8) -> bool {
        let n1 = self.nibbles(_mm256_loadu_si256(ip.cast()));
        let n2 = self.nibbles(_mm256_loadu_si256(ip.add(32).cast()));
        let n3 = self.nibbles(_mm256_loadu_si256(ip.add(64).cast()));
        let n4 = self.nibbles(_mm256_loadu_si256(ip.add(96).cast()));

        // Validate: a nibble > 15 has a bit set in 0xF0.
        let any = _mm256_or_si256(_mm256_or_si256(n1, n2), _mm256_or_si256(n3, n4));
        if _mm256_testz_si256(any, self.f0) == 0 {
            return false;
        }

        self.store_pair(n1, n2, op);
        self.store_pair(n3, n4, op.add(32));
        true
    }

    /// Decodes 64 hex characters to 32 bytes. Returns `false` if any is invalid.
    ///
    /// # Safety
    ///
    /// `ip` must be valid for reading 64 bytes and `op` for writing 32.
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn decode_64(&self, ip: *const u8, op: *mut u8) -> bool {
        let n1 = self.nibbles(_mm256_loadu_si256(ip.cast()));
        let n2 = self.nibbles(_mm256_loadu_si256(ip.add(32).cast()));
        if _mm256_testz_si256(_mm256_or_si256(n1, n2), self.f0) == 0 {
            return false;
        }

        self.store_pair(n1, n2, op);
        true
    }

    /// Decodes the first and last 16 characters of a `16..=32` character input in one pass.
    ///
    /// # Safety
    ///
    /// `ip` must be valid for reading `len` bytes and `op` for writing `len / 2`, with
    /// `16 <= len <= 32` and `len` even.
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn decode_16_32(&self, ip: *const u8, len: usize, op: *mut u8) -> bool {
        let v = _mm256_loadu2_m128i(ip.add(len - 16).cast(), ip.cast());
        let n = self.nibbles(v);
        if _mm256_testz_si256(n, self.f0) == 0 {
            return false;
        }

        let merged = _mm256_maddubs_epi16(n, self.weights);
        let packed = _mm256_packus_epi16(merged, merged);
        _mm_storel_epi64(op.cast(), _mm256_castsi256_si128(packed));
        _mm_storel_epi64(
            op.add((len - 16) / 2).cast(),
            _mm256_extracti128_si256::<1>(packed),
        );
        true
    }

    /// Decodes the first and last 8 characters of an `8..=15` character input in one pass.
    ///
    /// # Safety
    ///
    /// `ip` must be valid for reading `len` bytes and `op` for writing `len / 2`, with
    /// `8 <= len <= 15` and `len` even.
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn decode_8_15(&self, ip: *const u8, len: usize, op: *mut u8) -> bool {
        let first = _mm_loadl_epi64(ip.cast());
        let last = _mm_loadl_epi64(ip.add(len - 8).cast());
        let v = _mm256_castsi128_si256(_mm_unpacklo_epi64(first, last));
        let n = _mm256_castsi256_si128(self.nibbles(v));
        if _mm_testz_si128(n, _mm256_castsi256_si128(self.f0)) == 0 {
            return false;
        }

        let merged = _mm_maddubs_epi16(n, _mm256_castsi256_si128(self.weights));
        let packed = _mm_packus_epi16(merged, merged);
        op.cast::<i32>().write_unaligned(_mm_cvtsi128_si32(packed));
        op.add((len - 8) / 2)
            .cast::<i32>()
            .write_unaligned(_mm_extract_epi32::<1>(packed));
        true
    }
}
