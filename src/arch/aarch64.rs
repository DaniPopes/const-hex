#![allow(unsafe_op_in_unsafe_fn)]

use super::generic;
use crate::{Output, get_chars_table};
use core::arch::aarch64::*;
use core::mem::MaybeUninit;

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
    if !has_neon() {
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
    if !has_neon() {
        return generic::check(input);
    }
    unsafe { check_neon(input) }
}

/// `VALID[i]` is `0xFF` if `b'0' + i` is a hex digit. Indices past the table give 0.
const VALID: [u8; 64] = {
    let mut table = [0; 64];
    let mut i = 0;
    while i < 64 {
        if crate::HEX_DECODE_LUT[b'0' as usize + i] != crate::NIL {
            table[i] = 0xFF;
        }
        i += 1;
    }
    table
};

/// `NIB[i]` is `0x10 | nibble` if `b'0' + i` is a hex digit, else 0, like `VALID`.
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

#[target_feature(enable = "neon")]
pub(crate) unsafe fn check_neon(input: &[u8]) -> bool {
    let len = input.len();
    let ptr = input.as_ptr();

    if len < 16 {
        if len >= 8 {
            let v = vcombine_u8(vld1_u8(ptr), vld1_u8(ptr.add(len - 8)));
            vminvq_u8(valid_neon(v)) == 0xFF
        } else if len >= 4 {
            let first = ptr.cast::<u32>().read_unaligned() as u64;
            let last = ptr.add(len - 4).cast::<u32>().read_unaligned() as u64;
            let v = vcreate_u8(first | (last << 32));
            vminvq_u8(valid_neon(vcombine_u8(v, v))) == 0xFF
        } else {
            generic::check(input)
        }
    } else if len >= 64 {
        if !input.chunks_exact(64).all(|chunk| valid_64(chunk.as_ptr())) {
            return false;
        }
        // Re-check the last 64 bytes, overlapping what was already checked.
        len % 64 == 0 || valid_64(ptr.add(len - 64))
    } else if len > 32 {
        let first = vandq_u8(valid_neon(vld1q_u8(ptr)), valid_neon(vld1q_u8(ptr.add(16))));
        let last = vandq_u8(
            valid_neon(vld1q_u8(ptr.add(len - 32))),
            valid_neon(vld1q_u8(ptr.add(len - 16))),
        );
        vminvq_u8(vandq_u8(first, last)) == 0xFF
    } else {
        let first = valid_neon(vld1q_u8(ptr));
        let last = valid_neon(vld1q_u8(ptr.add(len - 16)));
        vminvq_u8(vandq_u8(first, last)) == 0xFF
    }
}

/// Per-byte validity mask: `0xFF` for hex digits, `0x00` otherwise.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn valid_neon(chunk: uint8x16_t) -> uint8x16_t {
    vqtbl4q_u8(
        vld1q_u8_x4(VALID.as_ptr()),
        vsubq_u8(chunk, vdupq_n_u8(b'0')),
    )
}

/// Checks 64 bytes.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn valid_64(ptr: *const u8) -> bool {
    let a = vandq_u8(valid_neon(vld1q_u8(ptr)), valid_neon(vld1q_u8(ptr.add(16))));
    let b = vandq_u8(
        valid_neon(vld1q_u8(ptr.add(32))),
        valid_neon(vld1q_u8(ptr.add(48))),
    );
    vminvq_u8(vandq_u8(a, b)) == 0xFF
}

/// Single-pass hex decode with validation.
///
/// Looks up each character in a 64-entry table that gives `0x10 | nibble` for hex digits and 0
/// otherwise, so a zero lane marks an invalid character. Pairs are merged with `vsliq_n_u8`,
/// which drops the marker bit.
#[inline]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    if cfg!(miri) || !has_neon() {
        return generic::decode_checked(input, output);
    }
    decode_checked_neon(input, output)
}

#[target_feature(enable = "neon")]
unsafe fn decode_checked_neon(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    debug_assert_eq!(output.len(), input.len() / 2);

    let len = input.len();
    let ip = input.as_ptr();
    let op = output.as_mut_ptr().cast::<u8>();

    // Every branch decodes the last chunk last, overlapping what was already decoded.
    if len >= 64 {
        for (i, chunk) in input.chunks_exact(64).enumerate() {
            if !decode_64(chunk.as_ptr(), op.add(i * 32)) {
                return false;
            }
        }
        match len % 64 {
            0 => true,
            33.. => decode_64(ip.add(len - 64), op.add((len - 64) / 2)),
            _ => decode_32(ip.add(len - 32), op.add((len - 32) / 2)),
        }
    } else if len >= 32 {
        decode_32(ip, op) && (len == 32 || decode_32(ip.add(len - 32), op.add((len - 32) / 2)))
    } else if len >= 16 {
        decode_16(ip, op) && decode_16(ip.add(len - 16), op.add((len - 16) / 2))
    } else if len >= 8 {
        decode_8(ip, op) && decode_8(ip.add(len - 8), op.add((len - 8) / 2))
    } else {
        generic::decode_checked(input, output)
    }
}

/// Converts ASCII hex to `0x10 | nibble`; any other byte becomes 0.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn nibbles(v: uint8x16_t) -> uint8x16_t {
    vqtbl4q_u8(vld1q_u8_x4(NIB.as_ptr()), vsubq_u8(v, vdupq_n_u8(b'0')))
}

/// Loads 32 characters as the high and low characters of each pair.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn load_pairs(ptr: *const u8) -> (uint8x16_t, uint8x16_t) {
    let a = vld1q_u8(ptr);
    let b = vld1q_u8(ptr.add(16));
    (vuzp1q_u8(a, b), vuzp2q_u8(a, b))
}

/// Decodes 64 hex characters to 32 bytes. Returns `false` if any is invalid.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn decode_64(ip: *const u8, op: *mut u8) -> bool {
    let (h0, l0) = load_pairs(ip);
    let (h1, l1) = load_pairs(ip.add(32));
    let (h0, l0, h1, l1) = (nibbles(h0), nibbles(l0), nibbles(h1), nibbles(l1));
    if vminvq_u8(vminq_u8(vminq_u8(h0, l0), vminq_u8(h1, l1))) == 0 {
        return false;
    }
    vst1q_u8(op, vsliq_n_u8(l0, h0, 4));
    vst1q_u8(op.add(16), vsliq_n_u8(l1, h1, 4));
    true
}

/// Decodes 32 hex characters to 16 bytes. Returns `false` if any is invalid.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn decode_32(ip: *const u8, op: *mut u8) -> bool {
    let (h, l) = load_pairs(ip);
    let (h, l) = (nibbles(h), nibbles(l));
    if vminvq_u8(vminq_u8(h, l)) == 0 {
        return false;
    }
    vst1q_u8(op, vsliq_n_u8(l, h, 4));
    true
}

/// Decodes 16 hex characters to 8 bytes. Returns `false` if any is invalid.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn decode_16(ip: *const u8, op: *mut u8) -> bool {
    let pairs = vld2_u8(ip);
    let n = nibbles(vcombine_u8(pairs.0, pairs.1));
    if vminvq_u8(n) == 0 {
        return false;
    }
    vst1_u8(op, vsli_n_u8(vget_high_u8(n), vget_low_u8(n), 4));
    true
}

/// Decodes 8 hex characters to 4 bytes. Returns `false` if any is invalid.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn decode_8(ip: *const u8, op: *mut u8) -> bool {
    let v = vcreate_u8(ip.cast::<u64>().read_unaligned());
    let n = nibbles(vcombine_u8(vuzp1_u8(v, v), vuzp2_u8(v, v)));
    if vminvq_u8(n) == 0 {
        return false;
    }
    let bytes = vsli_n_u8(vget_high_u8(n), vget_low_u8(n), 4);
    op.cast::<u32>()
        .write_unaligned(vget_lane_u32::<0>(vreinterpret_u32_u8(bytes)));
    true
}

#[inline]
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: &mut [MaybeUninit<u8>]) {
    if cfg!(miri) || !has_neon() {
        return generic::decode_unchecked(input, output);
    }
    decode_unchecked_neon(input, output);
}

#[target_feature(enable = "neon")]
unsafe fn decode_unchecked_neon(input: &[u8], output: &mut [MaybeUninit<u8>]) {
    generic::decode_unchecked_unaligned_chunks(input, output, |[v0, v1]: [uint8x16_t; 2]| {
        let n0 = unhex_neon(v0);
        let n1 = unhex_neon(v1);
        let uz = vuzpq_u8(n0, n1);
        vorrq_u8(vshlq_n_u8(uz.0, 4), uz.1)
    });
}

/// Converts ASCII hex bytes to nibble values: `(x >> 6) * 9 + (x & 0x0F)`.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn unhex_neon(x: uint8x16_t) -> uint8x16_t {
    let sr6 = vshrq_n_u8(x, 6);
    let low = vandq_u8(x, vdupq_n_u8(0x0F));
    let mul9 = vmulq_u8(sr6, vdupq_n_u8(9));
    vaddq_u8(mul9, low)
}
