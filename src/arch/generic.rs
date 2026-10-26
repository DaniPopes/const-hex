use crate::{HEX_DECODE_LUT, NIL, Output, byte2hex};
use core::mem::{MaybeUninit, size_of};

/// Set to `true` to use `check` + `decode_unchecked` for decoding. Otherwise uses `decode_checked`.
///
/// This should be set to `false` if `check` is not specialized.
#[allow(dead_code)]
pub(crate) const USE_CHECK_FN: bool = false;

/// Default encoding function.
///
/// # Safety
///
/// `output` must be at least `2 * input.len()` bytes long.
pub(crate) unsafe fn encode<const UPPER: bool>(mut input: &[u8], mut output: impl Output) {
    if cfg!(any(target_pointer_width = "64", target_arch = "wasm32")) {
        const ONES: u64 = 0x0101_0101_0101_0101;

        // Encodes 4 bytes into 8 hex digits in memory order.
        let swar = |bytes: [u8; 4]| {
            // Spread the 4 bytes to every other byte, then split each into its high and low nibble.
            let x = u32::from_le_bytes(bytes) as u64;
            let x = (x | (x << 16)) & 0x0000_FFFF_0000_FFFF;
            let x = (x | (x << 8)) & 0x00FF_00FF_00FF_00FF;
            let n = ((x >> 4) & 0x000F_000F_000F_000F) | ((x & 0x000F_000F_000F_000F) << 8);
            // Nibbles 10..=15 get the extra offset from '9' + 1 to 'a' (or 'A').
            let gt9 = ((n + ONES * 6) >> 4) & ONES;
            (n + ONES * b'0' as u64 + gt9 * if UPPER { 7 } else { 39 }).to_le_bytes()
        };
        while let &[a, b, c, d, ref rest @ ..] = input {
            output.write(&swar([a, b, c, d]));
            input = rest;
        }
    }
    for &byte in input {
        let (high, low) = byte2hex::<UPPER>(byte);
        output.write_byte(high);
        output.write_byte(low);
    }
}

/// Encodes unaligned chunks of `T` in `input` to `output` using `encode_chunk`.
///
/// The remainder is encoded using the generic [`encode`].
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn encode_unaligned_chunks<const UPPER: bool, T: Copy, U: Copy>(
    input: &[u8],
    mut output: impl Output,
    mut encode_chunk: impl FnMut(T) -> U,
) {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    let (chunks, remainder) = chunks_unaligned::<T>(input);
    for chunk in chunks {
        output.write(as_bytes(&encode_chunk(chunk)));
    }
    unsafe { encode::<UPPER>(remainder, output) };
}

/// Like [`encode_unaligned_chunks`], but with a custom remainder handler.
///
/// `encode_remainder` receives the remaining bytes and a mutable reference to the output
/// offset past the already-written chunks.
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn encode_unaligned_chunks_with<
    const UPPER: bool,
    T: Copy,
    U: Copy,
    O: Output,
>(
    input: &[u8],
    mut output: O,
    mut encode_chunk: impl FnMut(T) -> U,
    encode_remainder: impl FnOnce(&[u8], O),
) {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    let (chunks, remainder) = chunks_unaligned::<T>(input);
    for chunk in chunks {
        output.write(as_bytes(&encode_chunk(chunk)));
    }
    if !remainder.is_empty() {
        encode_remainder(remainder, output);
    }
}

/// Encodes at most one `T`-sized chunk, then scalar remainder.
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn encode_one_unaligned_chunk<const UPPER: bool, T: Copy, U: Copy>(
    input: &[u8],
    mut output: impl Output,
    encode_chunk: impl FnOnce(T) -> U,
) {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    if input.len() >= size_of::<T>() {
        debug_assert!(input.len() < size_of::<T>() * 2);
        let (l, r) = input.split_at(size_of::<T>());
        let chunk = l.as_ptr().cast::<T>().read_unaligned();
        output.write(as_bytes(&encode_chunk(chunk)));
        encode::<UPPER>(r, output);
    } else {
        encode::<UPPER>(input, output);
    }
}

/// Default check function.
#[inline]
pub(crate) const fn check(mut input: &[u8]) -> bool {
    while let &[
        a,
        b,
        c,
        d,
        e,
        f,
        g,
        h,
        a2,
        b2,
        c2,
        d2,
        e2,
        f2,
        g2,
        h2,
        ref rest @ ..,
    ] = input
    {
        if check_u64_mask(u64::from_ne_bytes([a, b, c, d, e, f, g, h]))
            | check_u64_mask(u64::from_ne_bytes([a2, b2, c2, d2, e2, f2, g2, h2]))
            != 0
        {
            return false;
        }
        input = rest;
    }
    if let &[a, b, c, d, e, f, g, h, ref rest @ ..] = input {
        if check_u64_mask(u64::from_ne_bytes([a, b, c, d, e, f, g, h])) != 0 {
            return false;
        }
        input = rest;
    }
    while let &[byte, ref rest @ ..] = input {
        if HEX_DECODE_LUT[byte as usize] == NIL {
            return false;
        }
        input = rest;
    }
    true
}

const ONES: u64 = 0x0101_0101_0101_0101;
const HIGH: u64 = ONES * 0x80;

/// Returns the high bit of each byte of `x` that is not an ASCII hex digit.
const fn check_u64_mask(x: u64) -> u64 {
    // Clear the high bits so that the additions below cannot carry between bytes.
    let x7 = x & !HIGH;
    let ge0 = x7 + ONES * (0x80 - b'0' as u64);
    let gt9 = x7 + ONES * (0x7F - b'9' as u64);
    let l = x7 | (ONES * 0x20);
    let gea = l + ONES * (0x80 - b'a' as u64);
    let gtf = l + ONES * (0x7F - b'f' as u64);
    let valid = (ge0 & !gt9) | (gea & !gtf);
    (x | !valid) & HIGH
}

/// Runs the given check function on unaligned chunks of `T` in `input`, with the remainder passed
/// to the generic [`check`].
#[inline]
#[allow(dead_code)]
pub(crate) fn check_unaligned_chunks<T: Copy>(
    input: &[u8],
    check_chunk: impl FnMut(T) -> bool,
) -> bool {
    check_unaligned_chunks_with(input, check_chunk, check)
}

/// Like [`check_unaligned_chunks`], but with a custom remainder handler.
#[inline]
#[allow(dead_code)]
pub(crate) fn check_unaligned_chunks_with<T: Copy>(
    input: &[u8],
    check_chunk: impl FnMut(T) -> bool,
    check_remainder: impl FnOnce(&[u8]) -> bool,
) -> bool {
    let (mut chunks, remainder) = chunks_unaligned(input);
    chunks.all(check_chunk) && (remainder.is_empty() || check_remainder(remainder))
}

/// Checks at most one `T`-sized chunk, then scalar remainder.
#[inline]
#[allow(dead_code)]
pub(crate) fn check_one_unaligned_chunk<T: Copy>(
    input: &[u8],
    check_chunk: impl FnOnce(T) -> bool,
) -> bool {
    if input.len() >= size_of::<T>() {
        debug_assert!(input.len() < size_of::<T>() * 2);
        let (l, r) = input.split_at(size_of::<T>());
        let chunk = unsafe { l.as_ptr().cast::<T>().read_unaligned() };
        check_chunk(chunk) && check(r)
    } else {
        check(input)
    }
}

/// Default checked decoding function.
///
/// # Safety
///
/// Assumes `output.len() == input.len() / 2`.
#[allow(dead_code)]
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [MaybeUninit<u8>]) -> bool {
    unsafe { decode_maybe_check::<true>(input, output) }
}

/// Default unchecked decoding function.
///
/// # Safety
///
/// Assumes `output.len() == input.len() / 2` and that the input is valid hex.
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: impl Output) {
    #[allow(unused_braces)] // False positive on older rust versions.
    let success = unsafe { decode_maybe_check::<{ cfg!(debug_assertions) }>(input, output) };
    debug_assert!(success);
}

/// Default decoding function. Checks input validity if `CHECK` is `true`, otherwise assumes it.
///
/// # Safety
///
/// Assumes `output.len() == input.len() / 2` and that the input is valid hex if `CHECK` is `true`.
#[inline(always)]
unsafe fn decode_maybe_check<const CHECK: bool>(input: &[u8], mut output: impl Output) -> bool {
    let l = output.remaining().unwrap_or(input.len() / 2);
    debug_assert_eq!(l, input.len() / 2);
    let mut i = 0;
    while i < l {
        let hex = unsafe { *input.get_unchecked(i * 2) };
        let high = HEX_DECODE_LUT[hex as usize];
        let hex = unsafe { *input.get_unchecked(i * 2 + 1) };
        let low = HEX_DECODE_LUT[hex as usize];
        if CHECK && (low | high) == NIL {
            return false;
        }
        output.write_byte(high << 4 | low);
        i += 1;
    }
    true
}

/// Decodes unaligned chunks of `U` in `input` to `output` using `decode_chunk`.
///
/// The remainder is decoded using the generic [`decode_unchecked`].
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn decode_unchecked_unaligned_chunks<T: Copy, U: Copy>(
    input: &[u8],
    mut output: impl Output,
    mut decode_chunk: impl FnMut(U) -> T,
) {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    let (chunks, remainder) = chunks_unaligned::<U>(input);
    for chunk in chunks {
        output.write(as_bytes(&decode_chunk(chunk)));
    }
    unsafe { decode_unchecked(remainder, output) };
}

/// Checked-decodes unaligned chunks of `U` in `input` to `output` using `decode_chunk`.
///
/// Returns `false` on the first invalid chunk. The remainder is decoded using [`decode_checked`].
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn decode_checked_unaligned_chunks<T: Copy, U: Copy>(
    input: &[u8],
    output: impl Output,
    decode_chunk: impl FnMut(U) -> Option<T>,
) -> bool {
    decode_checked_unaligned_chunks_with(input, output, decode_chunk, |remainder, out| unsafe {
        decode_maybe_check::<true>(remainder, out)
    })
}

/// Like [`decode_checked_unaligned_chunks`], but with a custom remainder handler.
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn decode_checked_unaligned_chunks_with<T: Copy, U: Copy, O: Output>(
    input: &[u8],
    mut output: O,
    mut decode_chunk: impl FnMut(U) -> Option<T>,
    decode_remainder: impl FnOnce(&[u8], O) -> bool,
) -> bool {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    let (chunks, remainder) = chunks_unaligned::<U>(input);
    for chunk in chunks {
        match decode_chunk(chunk) {
            Some(decoded) => output.write(as_bytes(&decoded)),
            None => return false,
        }
    }
    if !remainder.is_empty() {
        decode_remainder(remainder, output)
    } else {
        true
    }
}

/// Checked-decodes at most one `U`-sized chunk, then scalar remainder.
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn decode_checked_one_unaligned_chunk<T: Copy, U: Copy>(
    input: &[u8],
    mut output: impl Output,
    decode_chunk: impl FnOnce(U) -> Option<T>,
) -> bool {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    if input.len() >= size_of::<U>() {
        debug_assert!(input.len() < size_of::<U>() * 2);
        let (l, r) = input.split_at(size_of::<U>());
        let chunk = unsafe { l.as_ptr().cast::<U>().read_unaligned() };
        match decode_chunk(chunk) {
            Some(decoded) => {
                output.write(as_bytes(&decoded));
                unsafe { decode_maybe_check::<true>(r, output) }
            }
            None => false,
        }
    } else {
        unsafe { decode_maybe_check::<true>(input, output) }
    }
}

#[inline]
fn chunks_unaligned<T: Copy>(input: &[u8]) -> (impl ExactSizeIterator<Item = T> + '_, &[u8]) {
    let chunks = input.chunks_exact(core::mem::size_of::<T>());
    let remainder = chunks.remainder();
    (
        chunks.map(|chunk| unsafe { chunk.as_ptr().cast::<T>().read_unaligned() }),
        remainder,
    )
}

#[inline]
const fn as_bytes<T: Copy>(x: &T) -> &[u8] {
    unsafe { core::slice::from_raw_parts(x as *const _ as *const u8, size_of::<T>()) }
}
