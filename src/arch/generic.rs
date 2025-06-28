use crate::{byte2hex, HEX_DECODE_LUT, NIL};
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
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: &mut [u8]) {
    for (i, byte) in input.iter().enumerate() {
        let (high, low) = byte2hex::<UPPER>(*byte);
        unsafe {
            output[i * 2] = high;
            output[i * 2 + 1] = low;
        }
    }
}

/// Encodes unaligned chunks of `T` in `input` to `output` using `encode_chunk`.
///
/// The remainder is encoded using the generic [`encode`].
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn encode_unaligned_chunks<const UPPER: bool, T: Copy, U: Copy>(
    input: &[u8],
    output: &mut [u8],
    mut encode_chunk: impl FnMut(T) -> U,
) {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    let (chunks, remainder) = chunks_unaligned::<T>(input);
    let n_chunks = chunks.len();
    let chunk_output = output.as_mut_ptr().cast::<U>();
    for (i, chunk) in chunks.enumerate() {
        let out = encode_chunk(chunk);
        unsafe { chunk_output.add(i).write_unaligned(out) };
    }
    let remainder_output = unsafe { output.get_unchecked_mut(n_chunks * size_of::<U>()..) };
    unsafe { encode::<UPPER>(remainder, remainder_output) };
}

/// Default check function.
#[inline]
pub(crate) const fn check(mut input: &[u8]) -> bool {
    while let &[byte, ref rest @ ..] = input {
        if HEX_DECODE_LUT[byte as usize] == NIL {
            return false;
        }
        input = rest;
    }
    true
}

/// Runs the given check function on unaligned chunks of `T` in `input`, with the remainder passed
/// to the generic [`check`].
#[inline]
#[allow(dead_code)]
pub(crate) fn check_unaligned_chunks<T: Copy>(
    input: &[u8],
    check_chunk: impl FnMut(T) -> bool,
) -> bool {
    let (mut chunks, remainder) = chunks_unaligned(input);
    chunks.all(check_chunk) && check(remainder)
}

/// Default checked decoding function.
///
/// # Safety
///
/// Assumes `output.len() == input.len() / 2`.
pub(crate) unsafe fn decode_checked(input: &[u8], output: &mut [u8]) -> bool {
    unsafe { decode_maybe_check::<true>(input, output) }
}

/// Default unchecked decoding function.
///
/// # Safety
///
/// Assumes `output.len() == input.len() / 2` and that the input is valid hex.
pub(crate) unsafe fn decode_unchecked(input: &[u8], output: &mut [u8]) {
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
unsafe fn decode_maybe_check<const CHECK: bool>(input: &[u8], output: &mut [u8]) -> bool {
    macro_rules! next {
        ($var:ident, $i:expr) => {
            let hex = unsafe { *input.get_unchecked($i) };
            let $var = HEX_DECODE_LUT[hex as usize];
            if CHECK {
                if $var == NIL {
                    return false;
                }
            }
        };
    }

    debug_assert_eq!(output.len(), input.len() / 2);
    let mut i = 0;
    while i < output.len() {
        next!(high, i * 2);
        next!(low, i * 2 + 1);
        output[i] = high << 4 | low;
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
    output: &mut [u8],
    mut decode_chunk: impl FnMut(U) -> T,
) {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    let (chunks, remainder) = chunks_unaligned::<U>(input);
    let n_chunks = chunks.len();
    let chunk_output = output.as_mut_ptr().cast::<T>();
    for (i, chunk) in chunks.enumerate() {
        let out = decode_chunk(chunk);
        unsafe { chunk_output.add(i).write_unaligned(out) };
    }
    let remainder_output = unsafe { output.get_unchecked_mut(n_chunks * size_of::<T>()..) };
    unsafe { decode_unchecked(remainder, remainder_output) };
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
