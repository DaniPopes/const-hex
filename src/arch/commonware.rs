//! SIMD kernels written against [`commonware_simd`].
//!
//! [`dispatch`] selects the native x86-64-v4 (AVX-512 with GFNI, IFMA, and SHA) or AArch64
//! NEON with SHA2 backend at runtime. Every other CPU takes the scalar [`generic`] path.
//!
//! The kernels only need two byte-level primitives beyond the common instruction set: a
//! 16-byte table lookup ([`X86V4C::u8_shuffle128`] or [`Neon::u8_table_lookup`]), and on
//! x86 a way to move data between 128-bit groups, which the 32-bit lane permutes provide.

use super::generic;
use crate::{Output, HEX_CHARS_LOWER, HEX_CHARS_UPPER};
use commonware_simd::{dispatch, Neon, Operation, Simd, X86V4C};
use core::mem::MaybeUninit;

pub(crate) use generic::decode_unchecked;

pub(crate) const USE_CHECK_FN: bool = false;

/// Bytes in the widest vector of any backend.
const MAX_LANES: usize = 64;

#[inline]
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: impl Output) {
    dispatch(Encode::<_, UPPER> { input, output });
}

#[inline]
pub(crate) fn check(input: &[u8]) -> bool {
    dispatch(Check { input })
}

#[inline]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    debug_assert_eq!(output.len(), input.len() / 2);
    dispatch(Decode { input, output })
}

struct Encode<'a, O, const UPPER: bool> {
    input: &'a [u8],
    output: O,
}

impl<S: Simd, O: Output, const UPPER: bool> Operation<S> for Encode<'_, O, UPPER> {
    type Output = ();

    #[inline(always)]
    fn portable(self, _simd: S) {
        // SAFETY: The caller of `encode` guarantees the output length.
        unsafe { generic::encode::<UPPER>(self.input, self.output) }
    }

    #[inline(always)]
    fn x86_v4c(self, simd: S)
    where
        S: X86V4C,
    {
        let dup = simd.u8_load(&DUPLICATE);
        encode_simd::<S, O, UPPER>(
            simd,
            self.input,
            self.output,
            |table, indices| simd.u8_shuffle128(table, indices),
            |x| spread_x86(simd, x),
            [dup, dup],
        );
    }

    #[inline(always)]
    fn neon(self, simd: S)
    where
        S: Neon,
    {
        encode_simd::<S, O, UPPER>(
            simd,
            self.input,
            self.output,
            |table, indices| simd.u8_table_lookup(table, indices),
            |x| [x, x],
            [simd.u8_load(&DUPLICATE), simd.u8_load(&DUPLICATE[16..])],
        );
    }
}

struct Check<'a> {
    input: &'a [u8],
}

impl<S: Simd> Operation<S> for Check<'_> {
    type Output = bool;

    #[inline(always)]
    fn portable(self, _simd: S) -> bool {
        generic::check(self.input)
    }

    #[inline(always)]
    fn x86_v4c(self, simd: S) -> bool
    where
        S: X86V4C,
    {
        check_simd(simd, self.input, |table, indices| {
            simd.u8_shuffle128(table, indices)
        })
    }

    #[inline(always)]
    fn neon(self, simd: S) -> bool
    where
        S: Neon,
    {
        check_simd(simd, self.input, |table, indices| {
            simd.u8_table_lookup(table, indices)
        })
    }
}

struct Decode<'a, 'b> {
    input: &'a [u8],
    output: &'b mut [MaybeUninit<u8>],
}

impl<S: Simd> Operation<S> for Decode<'_, '_> {
    type Output = bool;

    #[inline(always)]
    fn portable(self, _simd: S) -> bool {
        // SAFETY: The caller of `decode_checked` guarantees the output length.
        unsafe { generic::decode_checked(self.input, self.output) }
    }

    #[inline(always)]
    fn x86_v4c(self, simd: S) -> bool
    where
        S: X86V4C,
    {
        decode_simd(
            simd,
            self.input,
            self.output,
            |table, indices| simd.u8_shuffle128(table, indices),
            |x| compact_x86(simd, x),
        )
    }

    #[inline(always)]
    fn neon(self, simd: S) -> bool
    where
        S: Neon,
    {
        decode_simd(
            simd,
            self.input,
            self.output,
            |table, indices| simd.u8_table_lookup(table, indices),
            |x| x,
        )
    }
}

/// Encodes `input` one vector at a time.
///
/// `shuffle(table, indices)` looks up each index in the 16-byte group of `table` it shares,
/// returning zero for indices with bit 7 set. `spread` maps one input vector to two vectors
/// whose 16-byte groups each hold, in their low or high half according to `dup`, the 8
/// input bytes encoded into that group of the output.
#[inline(always)]
fn encode_simd<S: Simd, O: Output, const UPPER: bool>(
    simd: S,
    input: &[u8],
    mut output: O,
    shuffle: impl Fn(S::U8, S::U8) -> S::U8 + Copy,
    spread: impl Fn(S::U8) -> [S::U8; 2],
    dup: [S::U8; 2],
) {
    let lanes = S::U8_LANES;
    let chars = simd.u8_load(&CHARS[UPPER as usize]);
    let even = simd.u8_load(&EVEN_MASK);
    let odd = simd.u8_load(&ODD_NIBBLE_MASK);
    let encode = |x: S::U8, dup: S::U8| {
        // Copy each byte to two lanes, then keep its high nibble in the even lane and its
        // low nibble in the odd lane.
        let x = shuffle(x, dup);
        let hi = simd.u8_and(simd.u8_shr::<4>(x), even);
        let lo = simd.u8_and(x, odd);
        shuffle(chars, simd.u8_xor(hi, lo))
    };

    let mut buf = [0; 2 * MAX_LANES];
    let mut chunks = input.chunks_exact(lanes);
    for chunk in &mut chunks {
        let [a, b] = spread(simd.u8_load(chunk));
        simd.u8_store(encode(a, dup[0]), &mut buf);
        simd.u8_store(encode(b, dup[1]), &mut buf[lanes..]);
        output.write(&buf[..2 * lanes]);
    }

    let mut rest = chunks.remainder();
    if rest.len() >= lanes / 2 {
        let (half, tail) = rest.split_at(lanes / 2);
        let [a, _] = spread(simd.u8_load_partial(half));
        simd.u8_store(encode(a, dup[0]), &mut buf);
        output.write(&buf[..lanes]);
        rest = tail;
    }
    // SAFETY: The caller of `encode` guarantees the output length.
    unsafe { generic::encode::<UPPER>(rest, output) };
}

#[inline(always)]
fn check_simd<S: Simd>(
    simd: S,
    input: &[u8],
    shuffle: impl Fn(S::U8, S::U8) -> S::U8 + Copy,
) -> bool {
    let lanes = S::U8_LANES;
    let mut valid = simd.u8_splat(VALID);
    let mut chunks = input.chunks_exact(lanes);
    for chunk in &mut chunks {
        let n = nibbles(simd, shuffle, simd.u8_load(chunk));
        valid = simd.u8_and(valid, n);
    }

    // Pad the tail with b'0', which is valid.
    let rest = chunks.remainder();
    if !rest.is_empty() {
        let pad = simd.u8_load(&ZERO_PAD[MAX_LANES - rest.len()..]);
        let x = simd.u8_xor(simd.u8_load_partial(rest), pad);
        valid = simd.u8_and(valid, nibbles(simd, shuffle, x));
    }

    all_valid(simd, valid)
}

/// Decodes and validates `input` two vectors at a time.
///
/// `compact` moves the decoded bytes, which are interleaved by 16-byte group, into memory
/// order. See [`encode_simd`] for `shuffle`.
#[inline(always)]
fn decode_simd<S: Simd>(
    simd: S,
    input: &[u8],
    mut output: &mut [MaybeUninit<u8>],
    shuffle: impl Fn(S::U8, S::U8) -> S::U8 + Copy,
    compact: impl Fn(S::U8) -> S::U8,
) -> bool {
    let lanes = S::U8_LANES;
    let low_nibble = simd.u8_splat(0x0F);
    let even = [simd.u8_load(&EVEN_LOW), simd.u8_load(&EVEN_HIGH)];
    let odd = [simd.u8_load(&ODD_LOW), simd.u8_load(&ODD_HIGH)];
    // Packs the nibble pairs of `n` into the low (`half = 0`) or high (`half = 1`) half of
    // each 16-byte group.
    let pack = |n: S::U8, half: usize| {
        let hi = shuffle(simd.u8_shl::<4>(n), even[half]);
        let lo = shuffle(simd.u8_and(n, low_nibble), odd[half]);
        simd.u8_xor(hi, lo)
    };

    let mut valid = simd.u8_splat(VALID);
    let mut buf = [0; MAX_LANES];
    let mut chunks = input.chunks_exact(2 * lanes);
    for chunk in &mut chunks {
        let a = nibbles(simd, shuffle, simd.u8_load(chunk));
        let b = nibbles(simd, shuffle, simd.u8_load(&chunk[lanes..]));
        valid = simd.u8_and(valid, simd.u8_and(a, b));
        simd.u8_store(compact(simd.u8_xor(pack(a, 0), pack(b, 1))), &mut buf);
        output.write(&buf[..lanes]);
    }

    // Decode one vector, then one half vector padded with b'0'.
    let mut rest = chunks.remainder();
    for len in [lanes, lanes / 2] {
        if rest.len() >= len {
            let (head, tail) = rest.split_at(len);
            let pad = simd.u8_load(&ZERO_PAD[MAX_LANES - len..]);
            let a = nibbles(simd, shuffle, simd.u8_xor(simd.u8_load_partial(head), pad));
            valid = simd.u8_and(valid, a);
            simd.u8_store(compact(pack(a, 0)), &mut buf);
            output.write(&buf[..len / 2]);
            rest = tail;
        }
    }

    // SAFETY: `output` has `rest.len() / 2` bytes left.
    all_valid(simd, valid) && unsafe { generic::decode_checked(rest, output) }
}

/// Marks a valid hex digit in the result of [`nibbles`].
const VALID: u8 = 0x10;

/// Maps each ASCII hex digit to `VALID | value`, and every other byte to a value without
/// `VALID`.
#[inline(always)]
fn nibbles<S: Simd>(simd: S, shuffle: impl Fn(S::U8, S::U8) -> S::U8, x: S::U8) -> S::U8 {
    let hi = simd.u8_shr::<4>(x);
    let lo = simd.u8_and(x, simd.u8_splat(0x0F));
    // The high nibble selects at most one of the digit and letter tables; the other
    // lookup gets bit 7 set in its index, which makes it return zero.
    let digit_index = simd.u8_xor(lo, shuffle(simd.u8_load(&DIGIT_DISABLE), hi));
    let alpha_index = simd.u8_xor(lo, shuffle(simd.u8_load(&ALPHA_DISABLE), hi));
    simd.u8_xor(
        shuffle(simd.u8_load(&DIGIT_VALUES), digit_index),
        shuffle(simd.u8_load(&ALPHA_VALUES), alpha_index),
    )
}

/// Returns whether every lane of the accumulated [`nibbles`] results has [`VALID`] set.
#[inline(always)]
fn all_valid<S: Simd>(simd: S, valid: S::U8) -> bool {
    let mut lanes = [VALID; MAX_LANES];
    simd.u8_store(valid, &mut lanes);
    let (words, _) = lanes.as_chunks::<8>();
    let all = words
        .iter()
        .fold(u64::MAX, |acc, &word| acc & u64::from_ne_bytes(word));
    let mask = u64::from_ne_bytes([VALID; 8]);
    all & mask == mask
}

/// Copies 16-byte groups `[0, 0, 1, 1]` and `[2, 2, 3, 3]` of `x` into two vectors.
#[inline(always)]
fn spread_x86<S: X86V4C>(simd: S, x: S::U8) -> [S::U8; 2] {
    let x = u8_to_u32(simd, x);
    [
        u32_to_u8(simd, simd.u32_shuffle_groups::<0x50>(x, x)),
        u32_to_u8(simd, simd.u32_shuffle_groups::<0xFA>(x, x)),
    ]
}

/// Gathers the low 8 bytes of each 16-byte group, then the high 8 bytes.
#[inline(always)]
fn compact_x86<S: X86V4C>(simd: S, x: S::U8) -> S::U8 {
    const INDICES: [usize; 16] = [0, 1, 4, 5, 8, 9, 12, 13, 2, 3, 6, 7, 10, 11, 14, 15];
    u32_to_u8(simd, simd.u32_permute(u8_to_u32(simd, x), &INDICES))
}

// `commonware-simd` has no register casts, so go through memory. These require byte and
// 32-bit vectors of the same width, and compile down to register moves.

#[inline(always)]
fn u8_to_u32<S: Simd>(simd: S, x: S::U8) -> S::U32 {
    debug_assert_eq!(S::U8_LANES, 4 * S::U32_LANES);
    let mut lanes = [0u32; MAX_LANES / 4];
    // SAFETY: `lanes` is `MAX_LANES` initialized bytes.
    let bytes = unsafe { core::slice::from_raw_parts_mut(lanes.as_mut_ptr().cast(), MAX_LANES) };
    simd.u8_store(x, bytes);
    simd.u32_load(&lanes)
}

#[inline(always)]
fn u32_to_u8<S: Simd>(simd: S, x: S::U32) -> S::U8 {
    debug_assert_eq!(S::U8_LANES, 4 * S::U32_LANES);
    let mut lanes = [0u32; MAX_LANES / 4];
    simd.u32_store(x, &mut lanes);
    // SAFETY: `lanes` is `MAX_LANES` initialized bytes.
    simd.u8_load(unsafe { core::slice::from_raw_parts(lanes.as_ptr().cast(), MAX_LANES) })
}

/// Repeats a 16-byte group across `MAX_LANES` bytes.
const fn splat16(group: [u8; 16]) -> [u8; MAX_LANES] {
    let mut out = [0; MAX_LANES];
    let mut i = 0;
    while i < MAX_LANES {
        out[i] = group[i % 16];
        i += 1;
    }
    out
}

/// Lowercase and uppercase digits.
const CHARS: [[u8; MAX_LANES]; 2] = [splat16(*HEX_CHARS_LOWER), splat16(*HEX_CHARS_UPPER)];

/// Duplicates bytes 0..8 of a group, then bytes 8..16 in the next group.
const DUPLICATE: [u8; MAX_LANES] = {
    let mut out = [0; MAX_LANES];
    let mut i = 0;
    while i < MAX_LANES {
        out[i] = (i % 32 / 2) as u8;
        i += 1;
    }
    out
};
const EVEN_MASK: [u8; MAX_LANES] = splat16([
    0xFF, 0, 0xFF, 0, 0xFF, 0, 0xFF, 0, 0xFF, 0, 0xFF, 0, 0xFF, 0, 0xFF, 0,
]);
const ODD_NIBBLE_MASK: [u8; MAX_LANES] = splat16([
    0, 0xF, 0, 0xF, 0, 0xF, 0, 0xF, 0, 0xF, 0, 0xF, 0, 0xF, 0, 0xF,
]);

const Z: u8 = 0x80;
const EVEN_LOW: [u8; MAX_LANES] = splat16([0, 2, 4, 6, 8, 10, 12, 14, Z, Z, Z, Z, Z, Z, Z, Z]);
const ODD_LOW: [u8; MAX_LANES] = splat16([1, 3, 5, 7, 9, 11, 13, 15, Z, Z, Z, Z, Z, Z, Z, Z]);
const EVEN_HIGH: [u8; MAX_LANES] = splat16([Z, Z, Z, Z, Z, Z, Z, Z, 0, 2, 4, 6, 8, 10, 12, 14]);
const ODD_HIGH: [u8; MAX_LANES] = splat16([Z, Z, Z, Z, Z, Z, Z, Z, 1, 3, 5, 7, 9, 11, 13, 15]);

/// Indexed by high nibble; zero only for b'0'..=b'?'.
const DIGIT_DISABLE: [u8; MAX_LANES] = splat16([Z, Z, Z, 0, Z, Z, Z, Z, Z, Z, Z, Z, Z, Z, Z, Z]);
/// Indexed by high nibble; zero only for b'@'..=b'O' and b'`'..=b'o'.
const ALPHA_DISABLE: [u8; MAX_LANES] = splat16([Z, Z, Z, Z, 0, Z, 0, Z, Z, Z, Z, Z, Z, Z, Z, Z]);
/// Indexed by low nibble.
const DIGIT_VALUES: [u8; MAX_LANES] = {
    let mut group = [0; 16];
    let mut i = 0;
    while i < 10 {
        group[i] = VALID | i as u8;
        i += 1;
    }
    splat16(group)
};
/// Indexed by low nibble.
const ALPHA_VALUES: [u8; MAX_LANES] = {
    let mut group = [0; 16];
    let mut i = 1;
    while i <= 6 {
        group[i] = VALID | (i as u8 + 9);
        i += 1;
    }
    splat16(group)
};

/// `ZERO_PAD[MAX_LANES - n..]` has zeros in its first `n` bytes and b'0' after.
const ZERO_PAD: [u8; 2 * MAX_LANES] = {
    let mut out = [0; 2 * MAX_LANES];
    let mut i = MAX_LANES;
    while i < 2 * MAX_LANES {
        out[i] = b'0';
        i += 1;
    }
    out
};

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use crate::impl_core::slice_assume_init;
    use alloc::{string::String, vec::Vec};
    use commonware_simd::emulated::{EmulatedArmV9, EmulatedNeon, EmulatedScalar, EmulatedX86V4C};

    // Emulated backends run the x86 and NEON kernels on any host.
    #[test]
    fn emulated_x86_v4c() {
        test_backend(EmulatedX86V4C);
    }

    #[test]
    fn emulated_neon() {
        test_backend(EmulatedNeon);
    }

    #[test]
    fn emulated_arm_v9() {
        test_backend(EmulatedArmV9);
    }

    #[test]
    fn emulated_scalar() {
        test_backend(EmulatedScalar);
    }

    fn test_backend<S: Simd>(simd: S) {
        let mut state = 0x243F_6A88_85A3_08D3_u64;
        let mut random = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        };
        // Every byte value in each part of the input: the two-vector loop, the one-vector
        // and half-vector steps, and the scalar tail, for 64- and 16-byte vectors.
        let mut positions = vec![
            0, 1, 7, 8, 15, 16, 17, 31, 32, 63, 64, 100, 127, 128, 129, 191, 192, 200, 223, 224,
            231, 232, 240, 247,
        ];
        let mut bytes = (0..=u8::MAX).collect::<Vec<_>>();
        let mut len_step = 1;
        // Miri is too slow for the exhaustive loops.
        if cfg!(miri) {
            positions = vec![0, 247];
            bytes = b"0g".to_vec();
            len_step = 97;
        }

        // Cover every chunk, half-chunk, and scalar tail split for 64-byte vectors.
        for len in (0..=300).step_by(len_step) {
            let input = (0..len).map(|_| random()).collect::<Vec<_>>();
            let lower = input.iter().map(|b| format!("{b:02x}")).collect::<String>();

            let mut output = vec![0; 2 * len];
            simd.execute(Encode::<_, false> {
                input: &input,
                output: output.as_mut_slice(),
            });
            assert_eq!(output, lower.as_bytes(), "{len}");
            simd.execute(Encode::<_, true> {
                input: &input,
                output: output.as_mut_slice(),
            });
            assert_eq!(output, lower.to_uppercase().as_bytes(), "{len}");

            let mixed = lower
                .bytes()
                .map(|c| {
                    if random() & 1 == 0 {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(decode(simd, &mixed).as_deref(), Some(&input[..]), "{len}");
            assert!(simd.execute(Check { input: &mixed }), "{len}");
            if let Some((_, odd)) = mixed.split_last() {
                assert!(simd.execute(Check { input: odd }), "{len}");
            }
        }

        let valid = (0..248)
            .map(|i| b"0123456789abcdefABCDEF"[i % 22])
            .collect::<Vec<_>>();
        for &pos in &positions {
            for &byte in &bytes {
                let mut input = valid.clone();
                input[pos] = byte;
                let is_hex = byte.is_ascii_hexdigit();
                assert_eq!(
                    simd.execute(Check { input: &input }),
                    is_hex,
                    "{pos} {byte}"
                );
                assert_eq!(decode(simd, &input).is_some(), is_hex, "{pos} {byte}");
            }
        }
    }

    fn decode<S: Simd>(simd: S, input: &[u8]) -> Option<Vec<u8>> {
        let mut output = vec![MaybeUninit::uninit(); input.len() / 2];
        simd.execute(Decode {
            input,
            output: &mut output,
        })
        .then(|| unsafe { slice_assume_init(&output) }.to_vec())
    }
}
