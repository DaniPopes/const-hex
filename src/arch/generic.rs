use crate::{byte2hex, HEX_DECODE_LUT, NIL};

/// Set to `true` to use `check` + `decode_unchecked` for decoding. Otherwise uses `decode_checked`.
///
/// This should be set to `false` if `check` is not specialized.
#[allow(dead_code)]
pub(crate) const USE_CHECK_FN: bool = false;

/// Default encoding function.
///
/// # Safety
///
/// `output` must be a valid pointer to at least `2 * input.len()` bytes.
pub(crate) unsafe fn encode<const UPPER: bool>(input: &[u8], output: *mut u8) {
    for (i, byte) in input.iter().enumerate() {
        let (high, low) = byte2hex::<UPPER>(*byte);
        unsafe {
            output.add(i * 2).write(high);
            output.add(i * 2 + 1).write(low);
        }
    }
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
    mut check_chunk: impl FnMut(T) -> bool,
) -> bool {
    let mut chunks = input.chunks_exact(core::mem::size_of::<T>());
    chunks.all(|chunk| check_chunk(unsafe { chunk.as_ptr().cast::<T>().read_unaligned() }))
        && check(chunks.remainder())
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
