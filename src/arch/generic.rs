use crate::{byte2hex, CheckResult, Output, HEX_DECODE_LUT, NIL};
use core::mem::size_of;

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
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], mut output: impl Output) {
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
///
/// Returns [`CheckResult::ok`] if all bytes are valid hex, or [`CheckResult::err`] with the index
/// of the first invalid byte.
#[inline]
pub(crate) const fn check(input: &[u8]) -> CheckResult {
    let mut i = 0;
    while i < input.len() {
        if HEX_DECODE_LUT[input[i] as usize] == NIL {
            return CheckResult::err(i);
        }
        i += 1;
    }
    CheckResult::ok()
}

/// Runs the given check function on unaligned chunks of `T` in `input`, with the remainder passed
/// to the generic [`check`].
#[inline]
#[allow(dead_code)]
pub(crate) fn check_unaligned_chunks<T: Copy>(
    input: &[u8],
    check_chunk: impl FnMut(T) -> bool,
) -> CheckResult {
    check_unaligned_chunks_with(input, check_chunk, check)
}

/// Like [`check_unaligned_chunks`], but with a custom remainder handler.
#[inline]
#[allow(dead_code)]
pub(crate) fn check_unaligned_chunks_with<T: Copy>(
    input: &[u8],
    mut check_chunk: impl FnMut(T) -> bool,
    check_remainder: impl FnOnce(&[u8]) -> CheckResult,
) -> CheckResult {
    let chunk_size = size_of::<T>();
    let (chunks, remainder) = chunks_unaligned(input);
    for (i, chunk) in chunks.enumerate() {
        if !check_chunk(chunk) {
            return CheckResult::err(i * chunk_size);
        }
    }
    if !remainder.is_empty() {
        let offset = input.len() - remainder.len();
        check_remainder(remainder).offset(offset)
    } else {
        CheckResult::ok()
    }
}

/// Checks at most one `T`-sized chunk, then scalar remainder.
#[inline]
#[allow(dead_code)]
pub(crate) fn check_one_unaligned_chunk<T: Copy>(
    input: &[u8],
    check_chunk: impl FnOnce(T) -> bool,
) -> CheckResult {
    if input.len() >= size_of::<T>() {
        debug_assert!(input.len() < size_of::<T>() * 2);
        let (l, r) = input.split_at(size_of::<T>());
        let chunk = unsafe { l.as_ptr().cast::<T>().read_unaligned() };
        if !check_chunk(chunk) {
            return CheckResult::err(0);
        }
        check(r).offset(size_of::<T>())
    } else {
        check(input)
    }
}

/// Default checked decoding function.
///
/// # Safety
///
/// Assumes `output.len() == input.len() / 2`.
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [u8]) -> CheckResult {
    unsafe { decode_maybe_check::<true>(input, output) }
}

/// Default unchecked decoding function.
///
/// # Safety
///
/// Assumes `output.len() == input.len() / 2` and that the input is valid hex.
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: impl Output) {
    #[allow(unused_braces)] // False positive on older rust versions.
    let result = unsafe { decode_maybe_check::<{ cfg!(debug_assertions) }>(input, output) };
    debug_assert!(result.is_ok());
}

/// Default decoding function. Checks input validity if `CHECK` is `true`, otherwise assumes it.
///
/// # Safety
///
/// Assumes `output.len() == input.len() / 2` and that the input is valid hex if `CHECK` is `true`.
#[inline(always)]
unsafe fn decode_maybe_check<const CHECK: bool>(
    input: &[u8],
    mut output: impl Output,
) -> CheckResult {
    macro_rules! next {
        ($var:ident, $i:expr) => {
            let hex = unsafe { *input.get_unchecked($i) };
            let $var = HEX_DECODE_LUT[hex as usize];
            if CHECK {
                if $var == NIL {
                    return CheckResult::err($i);
                }
            }
        };
    }

    let l = output.remaining().unwrap_or(input.len() / 2);
    debug_assert_eq!(l, input.len() / 2);
    let mut i = 0;
    while i < l {
        next!(high, i * 2);
        next!(low, i * 2 + 1);
        output.write_byte(high << 4 | low);
        i += 1;
    }
    CheckResult::ok()
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
/// Returns [`CheckResult::err`] on the first invalid chunk. The remainder is decoded using
/// [`decode_checked`].
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn decode_checked_unaligned_chunks<T: Copy, U: Copy>(
    input: &[u8],
    output: impl Output,
    decode_chunk: impl FnMut(U) -> Option<T>,
) -> CheckResult {
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
    decode_remainder: impl FnOnce(&[u8], O) -> CheckResult,
) -> CheckResult {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    let chunk_size = size_of::<U>();
    let (chunks, remainder) = chunks_unaligned::<U>(input);
    for (i, chunk) in chunks.enumerate() {
        match decode_chunk(chunk) {
            Some(decoded) => output.write(as_bytes(&decoded)),
            None => return CheckResult::err(i * chunk_size),
        }
    }
    if !remainder.is_empty() {
        let offset = input.len() - remainder.len();
        decode_remainder(remainder, output).offset(offset)
    } else {
        CheckResult::ok()
    }
}

/// Checked-decodes at most one `U`-sized chunk, then scalar remainder.
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn decode_checked_one_unaligned_chunk<T: Copy, U: Copy>(
    input: &[u8],
    mut output: impl Output,
    decode_chunk: impl FnOnce(U) -> Option<T>,
) -> CheckResult {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    if input.len() >= size_of::<U>() {
        debug_assert!(input.len() < size_of::<U>() * 2);
        let (l, r) = input.split_at(size_of::<U>());
        let chunk = unsafe { l.as_ptr().cast::<U>().read_unaligned() };
        match decode_chunk(chunk) {
            Some(decoded) => {
                output.write(as_bytes(&decoded));
                unsafe { decode_maybe_check::<true>(r, output) }.offset(size_of::<U>())
            }
            None => CheckResult::err(0),
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
