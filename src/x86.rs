#![allow(unsafe_op_in_unsafe_fn)]

use crate::generic;

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

pub(super) const USE_CHECK_FN: bool = true;
const CHUNK_SIZE: usize = core::mem::size_of::<__m128i>();

const T_MASK: i32 = 65535;

cpufeatures::new!(cpuid_sse2, "sse2");
cpufeatures::new!(cpuid_ssse3, "sse2", "ssse3");

#[inline]
pub(super) unsafe fn encode<const UPPER: bool>(input: &[u8], output: *mut u8) {
    if input.len() < CHUNK_SIZE || !cpuid_ssse3::get() {
        return generic::encode::<UPPER>(input, output);
    }
    _encode::<UPPER>(input, output);
}

#[target_feature(enable = "ssse3")]
unsafe fn _encode<const UPPER: bool>(input: &[u8], output: *mut u8) {
    // Load table and construct masks.
    let hex_table = _mm_loadu_si128(super::get_chars_table::<UPPER>().as_ptr().cast());
    let mask_lo = _mm_set1_epi8(0x0F);
    #[allow(clippy::cast_possible_wrap)]
    let mask_hi = _mm_set1_epi8(0xF0u8 as i8);

    let input_chunks = input.chunks_exact(CHUNK_SIZE);
    let input_remainder = input_chunks.remainder();

    let mut i = 0;
    for input_chunk in input_chunks {
        // Load input bytes and mask to nibbles.
        let input_bytes = _mm_loadu_si128(input_chunk.as_ptr().cast());
        let mut lo = _mm_and_si128(input_bytes, mask_lo);
        let mut hi = _mm_srli_epi32::<4>(_mm_and_si128(input_bytes, mask_hi));

        // Lookup the corresponding ASCII hex digit for each nibble.
        lo = _mm_shuffle_epi8(hex_table, lo);
        hi = _mm_shuffle_epi8(hex_table, hi);

        // Interleave the nibbles ([hi[0], lo[0], hi[1], lo[1], ...]).
        let hex_lo = _mm_unpacklo_epi8(hi, lo);
        let hex_hi = _mm_unpackhi_epi8(hi, lo);

        // Store result into the output buffer.
        _mm_storeu_si128(output.add(i).cast(), hex_lo);
        _mm_storeu_si128(output.add(i + CHUNK_SIZE).cast(), hex_hi);
        i += CHUNK_SIZE * 2;
    }

    if !input_remainder.is_empty() {
        generic::encode::<UPPER>(input_remainder, output.add(i));
    }
}

#[inline]
pub(super) fn check(input: &[u8]) -> bool {
    if input.len() < CHUNK_SIZE || !cpuid_sse2::get() {
        return generic::check(input);
    }
    unsafe { _check(input) }
}

#[target_feature(enable = "sse2")]
unsafe fn _check(input: &[u8]) -> bool {
    let ascii_zero = _mm_set1_epi8((b'0' - 1) as i8);
    let ascii_nine = _mm_set1_epi8((b'9' + 1) as i8);
    let ascii_ua = _mm_set1_epi8((b'A' - 1) as i8);
    let ascii_uf = _mm_set1_epi8((b'F' + 1) as i8);
    let ascii_la = _mm_set1_epi8((b'a' - 1) as i8);
    let ascii_lf = _mm_set1_epi8((b'f' + 1) as i8);

    let input_chunks = input.chunks_exact(CHUNK_SIZE);
    let input_remainder = input_chunks.remainder();
    for input_chunk in input_chunks {
        let unchecked = _mm_loadu_si128(input_chunk.as_ptr().cast());

        let gt0 = _mm_cmpgt_epi8(unchecked, ascii_zero);
        let lt9 = _mm_cmplt_epi8(unchecked, ascii_nine);
        let valid_digit = _mm_and_si128(gt0, lt9);

        let gtua = _mm_cmpgt_epi8(unchecked, ascii_ua);
        let ltuf = _mm_cmplt_epi8(unchecked, ascii_uf);

        let gtla = _mm_cmpgt_epi8(unchecked, ascii_la);
        let ltlf = _mm_cmplt_epi8(unchecked, ascii_lf);

        let valid_lower = _mm_and_si128(gtla, ltlf);
        let valid_upper = _mm_and_si128(gtua, ltuf);
        let valid_letter = _mm_or_si128(valid_lower, valid_upper);

        let ret = _mm_movemask_epi8(_mm_or_si128(valid_digit, valid_letter));
        if ret != T_MASK {
            return false;
        }
    }

    generic::check(input_remainder)
}

pub(super) use generic::decode_checked;
pub(super) use generic::decode_unchecked;
