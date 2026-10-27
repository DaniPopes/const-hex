use crate::{HEX_DECODE_LUT, NIL, Output, byte2hex};
use core::mem::{MaybeUninit, size_of};

/// Smallest piece of a tail for encoding and checking, in bytes.
const MIN_PIECE: usize = 4;

/// Smallest piece of a tail for decoding, in hex characters.
const MIN_DECODE_PIECE: usize = 8;

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

/// Encodes `input` to `output` in chunks of `T` using `encode_chunk`.
///
/// Inputs of at least `size_of::<T>()` bytes are encoded in whole chunks, with the last chunk
/// overlapping the one before it. Shorter inputs of at least [`MIN_PIECE`] bytes are encoded in
/// one chunk built from two overlapping loads. Anything shorter is encoded with [`encode`].
///
/// # Safety
///
/// `output` must be at least `2 * input.len()` bytes long.
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn encode_unaligned_chunks<const UPPER: bool, T: Copy, U: Copy>(
    input: &[u8],
    mut output: impl Output,
    encode_chunk: impl Fn(T) -> U,
) {
    debug_assert_eq!(size_of::<U>(), size_of::<T>() * 2);
    let len = input.len();
    debug_assert!(output.remaining().is_none_or(|n| n >= len * 2));
    let ip = input.as_ptr();
    let op = output.as_mut_ptr();

    let t = size_of::<T>();
    if len >= t {
        let mut i = 0;
        // Unrolled twice: a single-chunk loop runs up to 1.6x slower in some code layouts.
        while len - i >= t * 2 {
            unsafe {
                let (u0, u1) = (encode_chunk(read(ip, i)), encode_chunk(read(ip, i + t)));
                write(op, i * 2, u0);
                write(op, (i + t) * 2, u1);
            }
            i += t * 2;
        }
        if len - i >= t {
            unsafe { write(op, i * 2, encode_chunk(read(ip, i))) };
            i += t;
        }
        if i < len {
            // The last chunk, overlapping what is already encoded.
            unsafe { write(op, (len - t) * 2, encode_chunk(read(ip, len - t))) };
        }
    } else if len < MIN_PIECE {
        unsafe { encode::<UPPER>(input, output) };
    } else {
        unsafe {
            let chunk = by_piece::<T, MIN_PIECE, _>(len, |p| load_tail(ip, len, p));
            let encoded = encode_chunk(chunk);
            by_piece::<T, MIN_PIECE, _>(len, |p| {
                let (a, b) = tail_offsets(len, p);
                store_pair(encoded, op.add(a * 2), op.add(b * 2), p * 2)
            });
        }
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

/// Checks `input` in chunks of `T`.
///
/// `valid` returns a mask of the valid bytes of a chunk, `combine` merges the masks of several
/// chunks so they are tested together, and `all` tests whether a mask has no invalid byte.
///
/// Tails are handled as in [`encode_unaligned_chunks`], with inputs shorter than [`MIN_PIECE`]
/// bytes passed to [`check`].
#[inline]
#[allow(dead_code)]
pub(crate) fn check_unaligned_chunks<T: Copy, M: Copy>(
    input: &[u8],
    valid: impl Fn(T) -> M,
    combine: impl Fn(M, M) -> M,
    all: impl Fn(M) -> bool,
) -> bool {
    let len = input.len();
    let ip = input.as_ptr();
    let t = size_of::<T>();

    if len >= t {
        let mut i = 0;
        while len - i >= t * 4 {
            let [c0, c1, c2, c3] = unsafe { read::<[T; 4]>(ip, i) };
            let (v01, v23) = (combine(valid(c0), valid(c1)), combine(valid(c2), valid(c3)));
            if !all(combine(v01, v23)) {
                return false;
            }
            i += t * 4;
        }
        while len - i >= t {
            if !all(valid(unsafe { read(ip, i) })) {
                return false;
            }
            i += t;
        }
        // The last chunk, overlapping what is already checked.
        i == len || all(valid(unsafe { read(ip, len - t) }))
    } else if len < MIN_PIECE {
        check(input)
    } else {
        all(valid(unsafe {
            by_piece::<T, MIN_PIECE, _>(len, |p| load_tail(ip, len, p))
        }))
    }
}

/// Default checked decoding function.
///
/// # Safety
///
/// Assumes `output.len() == input.len() / 2`.
#[inline(always)]
pub(crate) unsafe fn decode_checked(input: &[u8], mut output: impl Output) -> bool {
    let l = output.remaining().unwrap_or(input.len() / 2);
    debug_assert_eq!(l, input.len() / 2);
    let mut i = 0;
    while i < l {
        let hex = unsafe { *input.get_unchecked(i * 2) };
        let high = HEX_DECODE_LUT[hex as usize];
        let hex = unsafe { *input.get_unchecked(i * 2 + 1) };
        let low = HEX_DECODE_LUT[hex as usize];
        if (low | high) == NIL {
            return false;
        }
        output.write_byte(high << 4 | low);
        i += 1;
    }
    true
}

/// Checked-decodes `input` to `output` in vectors of `V`.
///
/// `nibbles` converts the characters of a vector to nibbles, where an invalid character becomes a
/// value above 15. `combine` merges the nibbles of several vectors so they are tested together,
/// `all_valid` tests whether merged nibbles are all valid, and `pack` joins the nibbles of two
/// vectors into `T`, the decoded bytes of the first followed by those of the second. `T` is the
/// size of `V`.
///
/// Tails are handled as in [`encode_unaligned_chunks`], with inputs shorter than
/// [`MIN_DECODE_PIECE`] characters passed to [`decode_checked`]. Returns `false` on the first
/// invalid step, and may have written part of `output` by then.
///
/// # Safety
///
/// `output` must be exactly `input.len() / 2` bytes long, and `input.len()` must be even because
/// the tails overlap on whole hex pairs.
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn decode_checked_unaligned_chunks<V: Copy, N: Copy, T: Copy>(
    input: &[u8],
    mut output: impl Output,
    nibbles: impl Fn(V) -> N,
    combine: impl Fn(N, N) -> N,
    all_valid: impl Fn(N) -> bool,
    pack: impl Fn(N, N) -> T,
) -> bool {
    debug_assert_eq!(size_of::<V>(), size_of::<T>());
    let len = input.len();
    debug_assert_eq!(len % 2, 0);
    debug_assert!(output.remaining().is_none_or(|n| n == len / 2));
    let ip = input.as_ptr();
    let op = output.as_mut_ptr();
    let v = size_of::<V>();

    if len >= v {
        let mut i = 0;
        unsafe {
            while len - i >= v * 4 {
                let [v0, v1, v2, v3] = read::<[V; 4]>(ip, i);
                let (n0, n1, n2, n3) = (nibbles(v0), nibbles(v1), nibbles(v2), nibbles(v3));
                if !all_valid(combine(combine(n0, n1), combine(n2, n3))) {
                    return false;
                }
                write(op, i / 2, pack(n0, n1));
                write(op, i / 2 + v, pack(n2, n3));
                i += v * 4;
            }
            if len - i > v * 2 {
                let (n0, n1) = (nibbles(read(ip, i)), nibbles(read(ip, i + v)));
                if !all_valid(combine(n0, n1)) {
                    return false;
                }
                write(op, i / 2, pack(n0, n1));
                i += v * 2;
            }
            if len - i > v {
                // The last two vectors, overlapping what is already decoded.
                let (a, b) = tail_offsets(len, v);
                let (n0, n1) = (nibbles(read(ip, a)), nibbles(read(ip, b)));
                if !all_valid(combine(n0, n1)) {
                    return false;
                }
                store_pair(pack(n0, n1), op.add(a / 2), op.add(b / 2), v / 2);
            } else if i < len {
                // The last vector, overlapping what is already decoded.
                let n = nibbles(read(ip, len - v));
                if !all_valid(n) {
                    return false;
                }
                let dst = op.add((len - v) / 2);
                store_pair(pack(n, n), dst, dst.add(v / 4), v / 4);
            }
        }
        true
    } else if len < MIN_DECODE_PIECE {
        unsafe { decode_checked(input, output) }
    } else {
        unsafe {
            let n = nibbles(by_piece::<V, MIN_DECODE_PIECE, _>(len, |p| {
                load_tail(ip, len, p)
            }));
            if !all_valid(n) {
                return false;
            }
            let decoded = pack(n, n);
            by_piece::<V, MIN_DECODE_PIECE, _>(len, |p| {
                let (a, b) = tail_offsets(len, p);
                store_pair(decoded, op.add(a / 2), op.add(b / 2), p / 2)
            });
        }
        true
    }
}

/// Calls `f` with the size of the pieces that make up the last chunk of `len` bytes, as a constant.
///
/// This is half a chunk, or if the input is shorter than that, the largest of its halves.
#[inline(always)]
fn by_piece<T, const MIN: usize, R>(len: usize, f: impl FnOnce(usize) -> R) -> R {
    const { assert!(size_of::<T>() <= 64 && size_of::<T>() / 8 <= MIN) };
    let half = size_of::<T>() / 2;
    if len >= half {
        f(half)
    } else if len >= half / 2 {
        f(half / 2)
    } else {
        f(half / 4)
    }
}

/// Reads a `T` at `offset` bytes from `ptr`.
#[inline(always)]
const unsafe fn read<T>(ptr: *const u8, offset: usize) -> T {
    unsafe { ptr.add(offset).cast::<T>().read_unaligned() }
}

/// Writes a `T` at `offset` bytes from `ptr`.
#[inline(always)]
const unsafe fn write<T>(ptr: *mut u8, offset: usize, value: T) {
    unsafe { ptr.add(offset).cast::<T>().write_unaligned(value) }
}

/// Offsets of the two `piece`-sized pieces of the last chunk of `len` bytes.
///
/// The second piece ends at `len`. The first piece ends where the second starts, unless the input
/// is shorter than two pieces, in which case it starts at `0` and the pieces overlap.
#[inline(always)]
const fn tail_offsets(len: usize, piece: usize) -> (usize, usize) {
    (len.saturating_sub(piece * 2), len - piece)
}

/// Loads the pieces of the last chunk of `len` bytes into a `T` with [`load_pair`].
#[inline(always)]
unsafe fn load_tail<T: Copy>(ip: *const u8, len: usize, piece: usize) -> T {
    let (a, b) = tail_offsets(len, piece);
    unsafe { load_pair(ip.add(a), ip.add(b), piece) }
}

/// Loads a `T` made of `piece` bytes at `a`, then `piece` bytes at `b`, repeated to fill `T`.
#[inline(always)]
unsafe fn load_pair<T: Copy>(a: *const u8, b: *const u8, piece: usize) -> T {
    let mut chunk = MaybeUninit::<T>::uninit();
    let dst = chunk.as_mut_ptr().cast::<u8>();
    if piece == 4 {
        // Combining the pieces as integers keeps LLVM from merging the two loads into a gather.
        let (x, y) = unsafe {
            (
                u64::from(a.cast::<u32>().read_unaligned()),
                u64::from(b.cast::<u32>().read_unaligned()),
            )
        };
        let pair = if cfg!(target_endian = "little") {
            x | y << 32
        } else {
            x << 32 | y
        };
        let mut i = 0;
        while i < size_of::<T>() {
            unsafe { dst.add(i).cast::<u64>().write_unaligned(pair) };
            i += 8;
        }
    } else {
        let mut i = 0;
        while i < size_of::<T>() {
            unsafe {
                a.copy_to_nonoverlapping(dst.add(i), piece);
                b.copy_to_nonoverlapping(dst.add(i + piece), piece);
            }
            i += piece * 2;
        }
    }
    unsafe { chunk.assume_init() }
}

/// Stores the first `len` bytes of `chunk` at `a`, and the next `len` bytes at `b`.
#[inline(always)]
const unsafe fn store_pair<T: Copy>(chunk: T, a: *mut u8, b: *mut u8, len: usize) {
    let src = (&chunk as *const T).cast::<u8>();
    unsafe {
        src.copy_to_nonoverlapping(a, len);
        src.add(len).copy_to_nonoverlapping(b, len);
    }
}
