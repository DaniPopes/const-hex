use crate::{HEX_DECODE_LUT, NIL, Output, byte2hex};
use core::mem::{MaybeUninit, size_of};

/// Smallest piece of a tail for encoding and checking, in bytes.
const MIN_PIECE: usize = 4;

/// Smallest piece of a tail for decoding, in hex characters.
const MIN_DECODE_PIECE: usize = MIN_PIECE * 2;

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

/// A SIMD vector of bytes, with the operations the SIMD kernels below are built from.
///
/// Each backend implements this for its vector type and passes the kernels to the chunk helpers.
///
/// # Safety
///
/// Every method is `unsafe` because an implementation may need a CPU feature that the caller must
/// have enabled: `hex_digits` and `pack` of `__m128i` need SSSE3, and every method of `__m256i`
/// needs AVX2.
#[allow(dead_code)]
pub(crate) trait Vector: Copy {
    /// Returns a vector with every byte set to `x`.
    unsafe fn splat(x: u8) -> Self;

    /// Returns `self & other`.
    unsafe fn and(self, other: Self) -> Self;

    /// Returns `self | other`.
    unsafe fn or(self, other: Self) -> Self;

    /// Returns `self - other` for each byte, wrapping on overflow.
    unsafe fn sub(self, other: Self) -> Self;

    /// Returns `self + other` for each byte, saturating at 255.
    unsafe fn adds(self, other: Self) -> Self;

    /// Returns `self - other` for each byte, saturating at 0.
    unsafe fn subs(self, other: Self) -> Self;

    /// Returns the smaller of `self` and `other` for each byte.
    unsafe fn min(self, other: Self) -> Self;

    /// Returns each byte shifted right by 4 bits.
    unsafe fn shr4(self) -> Self;

    /// Returns the ASCII hex digit of each byte, which must be at most 15.
    unsafe fn hex_digits<const UPPER: bool>(self) -> Self;

    /// Interleaves the bytes of `a` and `b`: `a[0], b[0], a[1], b[1], ...`, returned as two vectors.
    unsafe fn zip(a: Self, b: Self) -> [Self; 2];

    /// Joins pairs of nibbles into bytes: the even bytes of `a` then `b` are the high nibbles and
    /// the odd bytes the low nibbles, and the result is the bytes of `a` followed by those of `b`.
    ///
    /// Each byte must be at most 15, unless the implementation says that it accepts more.
    unsafe fn pack(a: Self, b: Self) -> Self;

    /// Returns whether every byte is zero.
    unsafe fn is_zero(self) -> bool;

    /// Loads a vector that starts with `piece` bytes at `a`, then `piece` bytes at `b`. The other
    /// bytes are unspecified, but must stay valid hex digits for `check`, so they are the pieces
    /// again or `b'0'`.
    ///
    /// # Safety
    ///
    /// `a` and `b` must be valid for reads of `piece` bytes, and `piece` must be a power of two
    /// from 4 up to half the size of the vector.
    #[inline(always)]
    unsafe fn load_pair(a: *const u8, b: *const u8, piece: usize) -> Self {
        let mut chunk = MaybeUninit::<Self>::uninit();
        let dst = chunk.as_mut_ptr().cast::<u8>();
        if piece == 4 {
            // Combining the pieces as integers keeps LLVM from merging the two loads into a
            // gather.
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
            while i < size_of::<Self>() {
                unsafe { dst.add(i).cast::<u64>().write_unaligned(pair) };
                i += 8;
            }
        } else {
            let mut i = 0;
            while i < size_of::<Self>() {
                unsafe {
                    a.copy_to_nonoverlapping(dst.add(i), piece);
                    b.copy_to_nonoverlapping(dst.add(i + piece), piece);
                }
                i += piece * 2;
            }
        }
        unsafe { chunk.assume_init() }
    }

    /// Stores the first `len` bytes at `a`, and the next `len` bytes at `b`.
    ///
    /// # Safety
    ///
    /// `a` and `b` must be valid for writes of `len` bytes, and `2 * len` must not exceed the size
    /// of the vector.
    #[inline(always)]
    unsafe fn store_pair(self, a: *mut u8, b: *mut u8, len: usize) {
        let src = (&self as *const Self).cast::<u8>();
        unsafe {
            src.copy_to_nonoverlapping(a, len);
            src.add(len).copy_to_nonoverlapping(b, len);
        }
    }
}

/// Implements [`Vector`] for `$ty`. Each `$method = $f` defines a method that returns
/// `$f(self, other)`, and the remaining methods follow.
#[allow(unused_macros)]
macro_rules! impl_vector {
    ($ty:ty; $($method:ident = $f:path),* $(,)?; $($rest:tt)*) => {
        impl Vector for $ty {
            $(
                #[inline(always)]
                unsafe fn $method(self, other: Self) -> Self {
                    $f(self, other)
                }
            )*

            $($rest)*
        }
    };
}
#[allow(unused_imports)]
pub(crate) use impl_vector;

/// Encodes a chunk to hex: the hex digits of its high nibbles interleaved with those of its low
/// nibbles.
///
/// The kernels are `#[inline(always)]` and have no `#[target_feature]`, so a backend must pass
/// them from closures written inside its `#[target_feature]` function: only those closures inherit
/// the features that let the intrinsics inline.
#[inline(always)]
#[allow(dead_code)]
pub(crate) unsafe fn encode_kernel<const UPPER: bool, V: Vector>(chunk: V) -> [V; 2] {
    unsafe {
        let hi = chunk.shr4().hex_digits::<UPPER>();
        let lo = chunk.and(V::splat(0x0f)).hex_digits::<UPPER>();
        V::zip(hi, lo)
    }
}

/// Returns a vector that is zero exactly in the bytes of `chunk` that are ASCII hex digits.
///
/// A byte is a hex digit iff `x - b'0' <= 9` or `(x | 0x20) - b'a' <= 5`, with wrapping
/// subtraction, so the smaller of the two saturating excesses over the limits is zero exactly for
/// hex digits.
#[inline(always)]
#[allow(dead_code)]
pub(crate) unsafe fn check_kernel<V: Vector>(chunk: V) -> V {
    unsafe {
        let digit = chunk.sub(V::splat(b'0')).subs(V::splat(9));
        let letter = chunk
            .or(V::splat(0x20))
            .sub(V::splat(b'a'))
            .subs(V::splat(5));
        digit.min(letter)
    }
}

/// Converts the hex digits of `chunk` to nibbles with Muła & Langdale's algorithm #3: valid bytes
/// become `0..=15` and any other byte becomes a value above 15.
///
/// <http://0x80.pl/notesen/2022-01-17-validating-hex-parse.html>
#[inline(always)]
#[allow(dead_code)]
pub(crate) unsafe fn decode_kernel<V: Vector>(chunk: V) -> V {
    unsafe {
        // Digits '0'..'9' → 0..9, others > 15.
        let digit = chunk
            .sub(V::splat(b'9' + 1))
            .subs(V::splat(6))
            .sub(V::splat(0xF0));
        // Letters 'A'..'F' and 'a'..'f' → 10..15, others > 15.
        let letter = chunk
            .and(V::splat(0xDF))
            .sub(V::splat(b'A'))
            .adds(V::splat(10));
        digit.min(letter)
    }
}

/// Returns whether every nibble from [`decode_kernel`] is valid, that is at most 15.
#[inline(always)]
#[allow(dead_code)]
pub(crate) unsafe fn decode_all_valid<V: Vector>(nibbles: V) -> bool {
    unsafe { nibbles.and(V::splat(0xF0)).is_zero() }
}

/// Encodes `input` to `output` in chunks of `T` using `encode_chunk`.
///
/// Inputs of at least `size_of::<T>()` bytes are encoded in whole chunks, with the last one or two
/// chunks overlapping the ones before them. Shorter inputs of at least [`MIN_PIECE`] bytes are
/// encoded in one chunk built from two overlapping loads. Anything shorter is encoded with
/// [`encode`].
///
/// `UNROLL` is the number of chunks per iteration of the main loop, `1` or `2`. With `1`,
/// `FINISH_IN_LOOP` selects whether the loop encodes every whole chunk and an exact multiple of the
/// chunk size returns without a final step, or the loop stops one chunk early and a single final
/// step, shared with the short-input tail, always encodes the last chunk. These shapes, and
/// unrolling, are a per-backend choice because each is the fastest in some code generators and
/// slower in others (see the callers); `FINISH_IN_LOOP` has no effect with `UNROLL` of `2`.
///
/// # Safety
///
/// `output` must be at least `2 * input.len()` bytes long.
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn encode_unaligned_chunks<
    const UPPER: bool,
    const UNROLL: usize,
    const FINISH_IN_LOOP: bool,
    T: Vector,
>(
    input: &[u8],
    mut output: impl Output,
    encode_chunk: impl Fn(T) -> [T; 2],
) {
    const { assert!(UNROLL == 1 || UNROLL == 2) };
    let len = input.len();
    debug_assert!(output.remaining().is_none_or(|n| n >= len * 2));
    let ip = input.as_ptr();
    let op = output.as_mut_ptr();
    let t = size_of::<T>();

    if len >= t {
        let (mut p, mut o) = (ip, op);
        if UNROLL == 2 {
            if len >= t * 2 {
                let end = unsafe { ip.add(len - t * 2) };
                while p <= end {
                    unsafe {
                        let (u0, u1) = (encode_chunk(read(p, 0)), encode_chunk(read(p, t)));
                        write(o, 0, u0);
                        write(o, t * 2, u1);
                        p = p.add(t * 2);
                        o = o.add(t * 4);
                    }
                }
            }
            let done = p.addr() - ip.addr();
            if len - done > t {
                // The last two chunks, overlapping each other or what is already encoded.
                let (a, b) = tail_offsets(len, t);
                unsafe {
                    let (u0, u1) = (encode_chunk(read(ip, a)), encode_chunk(read(ip, b)));
                    write(op, a * 2, u0);
                    write(op, b * 2, u1);
                }
                return;
            }
            if done == len {
                return;
            }
        } else {
            let end = unsafe { ip.add(len - t) };
            while p < end || (FINISH_IN_LOOP && p == end) {
                unsafe {
                    write(o, 0, encode_chunk(read(p, 0)));
                    p = p.add(t);
                    o = o.add(t * 2);
                }
            }
            if FINISH_IN_LOOP {
                if p.addr() - ip.addr() == len {
                    return;
                }
                // The last chunk, overlapping what is already encoded.
                unsafe { write(op, (len - t) * 2, encode_chunk(read(ip, len - t))) };
                return;
            }
        }
    }

    // The last chunk, ending at `len`. Below one chunk it is built from two overlapping half-chunk
    // loads, and either way its encoded halves are stored so they end at `len * 2`.
    if len >= t / 2 {
        let chunk = if len >= t {
            unsafe { read(ip, len - t) }
        } else {
            unsafe { load_tail(ip, len, t / 2) }
        };
        let [x, y] = encode_chunk(chunk);
        let (a, b) = tail_offsets(len * 2, t);
        unsafe {
            write(op, a, x);
            write(op, b, y);
        }
    } else if let Some(chunk) =
        by_piece::<T, MIN_PIECE, _>(len, |p| unsafe { load_tail(ip, len, p) })
    {
        // The pieces are at most a quarter of a chunk, so their encoded halves are in the first
        // vector.
        let [encoded, _] = encode_chunk(chunk);
        let stored = by_piece::<T, MIN_PIECE, _>(len, |p| unsafe {
            let (a, b) = tail_offsets(len, p);
            encoded.store_pair(op.add(a * 2), op.add(b * 2), p * 2)
        });
        debug_assert!(stored.is_some());
    } else {
        unsafe { encode::<UPPER>(input, output) };
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
/// Inputs of at least `size_of::<T>()` bytes are checked in groups of four chunks, then by the size
/// of the remainder in one more group of four, two or one chunk that overlaps the ones before it.
/// An input of up to four chunks is checked as one, two or four chunks. Shorter inputs of at least
/// [`MIN_PIECE`] bytes are checked in one chunk built from two overlapping loads. Anything shorter
/// is passed to [`check`].
#[inline]
#[allow(dead_code)]
pub(crate) fn check_unaligned_chunks<T: Vector, M: Copy>(
    input: &[u8],
    valid: impl Fn(T) -> M,
    combine: impl Fn(M, M) -> M,
    all: impl Fn(M) -> bool,
) -> bool {
    let len = input.len();
    let ip = input.as_ptr();
    let t = size_of::<T>();

    if len >= t {
        let mut p = ip;
        if len >= t * 4 {
            let end = unsafe { ip.add(len - t * 4) };
            while p <= end {
                let [c0, c1, c2, c3] = unsafe { read::<[T; 4]>(p, 0) };
                let (v01, v23) = (combine(valid(c0), valid(c1)), combine(valid(c2), valid(c3)));
                if !all(combine(v01, v23)) {
                    return false;
                }
                p = unsafe { p.add(t * 4) };
            }
        }
        let rem = len - (p.addr() - ip.addr());
        if rem > t * 2 {
            // The last four chunks, overlapping each other or what is already checked. The first
            // two chunks start at the beginning of the input if it is shorter than four chunks.
            let a = len.saturating_sub(t * 4);
            let (v01, v23) = (
                combine(
                    valid(unsafe { read(ip, a) }),
                    valid(unsafe { read(ip, a + t) }),
                ),
                combine(
                    valid(unsafe { read(ip, len - t * 2) }),
                    valid(unsafe { read(ip, len - t) }),
                ),
            );
            all(combine(v01, v23))
        } else if rem > t {
            // The last two chunks.
            let (a, b) = tail_offsets(len, t);
            all(combine(
                valid(unsafe { read(ip, a) }),
                valid(unsafe { read(ip, b) }),
            ))
        } else {
            // The last chunk.
            rem == 0 || all(valid(unsafe { read(ip, len - t) }))
        }
    } else if let Some(chunk) =
        by_piece::<T, MIN_PIECE, _>(len, |p| unsafe { load_tail(ip, len, p) })
    {
        all(valid(chunk))
    } else {
        check(input)
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
/// Inputs of at least `size_of::<V>()` characters are decoded in groups of four vectors, and the
/// remainder in one last group of four, two or one vector that overlaps the groups before it.
/// Shorter inputs of at least [`MIN_DECODE_PIECE`] characters are decoded in one vector built from
/// two overlapping loads. Anything shorter is passed to [`decode_checked`]. Returns `false` on the
/// first invalid group, and may have written part of `output` by then.
///
/// # Safety
///
/// `output` must be exactly `input.len() / 2` bytes long, and `input.len()` must be even because
/// the tails overlap on whole hex pairs.
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn decode_checked_unaligned_chunks<V: Vector, N: Copy, T: Vector>(
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
        let (mut p, mut o) = (ip, op);
        unsafe {
            if len >= v * 4 {
                let end = ip.add(len - v * 4);
                while p <= end {
                    let [v0, v1, v2, v3] = read::<[V; 4]>(p, 0);
                    let (n0, n1, n2, n3) = (nibbles(v0), nibbles(v1), nibbles(v2), nibbles(v3));
                    if !all_valid(combine(combine(n0, n1), combine(n2, n3))) {
                        return false;
                    }
                    write(o, 0, pack(n0, n1));
                    write(o, v, pack(n2, n3));
                    p = p.add(v * 4);
                    o = o.add(v * 2);
                }
            }
            let rem = len - (p.addr() - ip.addr());
            if rem > v * 2 {
                // The last four vectors, overlapping each other or what is already decoded. The
                // first two vectors start at the beginning of the input if it is shorter than four
                // vectors.
                let a = len.saturating_sub(v * 4);
                let (n0, n1) = (nibbles(read(ip, a)), nibbles(read(ip, a + v)));
                let (n2, n3) = (nibbles(read(ip, len - v * 2)), nibbles(read(ip, len - v)));
                if !all_valid(combine(combine(n0, n1), combine(n2, n3))) {
                    return false;
                }
                write(op, a / 2, pack(n0, n1));
                write(op, (len - v * 2) / 2, pack(n2, n3));
            } else if rem > v {
                // The last two vectors, overlapping each other or what is already decoded.
                let (a, b) = tail_offsets(len, v);
                let (n0, n1) = (nibbles(read(ip, a)), nibbles(read(ip, b)));
                if !all_valid(combine(n0, n1)) {
                    return false;
                }
                if len >= v * 2 {
                    write(op, a / 2, pack(n0, n1));
                } else {
                    pack(n0, n1).store_pair(op.add(a / 2), op.add(b / 2), v / 2);
                }
            } else if rem > 0 {
                // The last vector, overlapping what is already decoded, if anything.
                let n = nibbles(read(ip, len - v));
                if !all_valid(n) {
                    return false;
                }
                store_prefix(pack(n, n), op.add((len - v) / 2), v / 2);
            }
        }
        true
    } else if let Some(n) =
        by_piece::<V, MIN_DECODE_PIECE, _>(len, |p| unsafe { nibbles(load_tail(ip, len, p)) })
    {
        if !all_valid(n) {
            return false;
        }
        let decoded = pack(n, n);
        let stored = by_piece::<V, MIN_DECODE_PIECE, _>(len, |p| unsafe {
            let (a, b) = tail_offsets(len, p);
            decoded.store_pair(op.add(a / 2), op.add(b / 2), p / 2)
        });
        debug_assert!(stored.is_some());
        true
    } else {
        unsafe { decode_checked(input, output) }
    }
}

/// Calls `f` with the size of the pieces that make up the last chunk of `len` bytes, as a constant.
///
/// This is the largest halving of the chunk size that fits in `len` and is at least `MIN`, down to
/// an eighth of a chunk. Returns `None` if there is none.
#[inline(always)]
fn by_piece<T, const MIN: usize, R>(len: usize, f: impl FnOnce(usize) -> R) -> Option<R> {
    let half = size_of::<T>() / 2;
    if len >= half {
        Some(f(half))
    } else if half / 2 >= MIN && len >= half / 2 {
        Some(f(half / 2))
    } else if half / 4 >= MIN && len >= half / 4 {
        Some(f(half / 4))
    } else {
        None
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

/// Loads the pieces of the last chunk of `len` bytes into a `T` with [`Vector::load_pair`].
#[inline(always)]
unsafe fn load_tail<T: Vector>(ip: *const u8, len: usize, piece: usize) -> T {
    let (a, b) = tail_offsets(len, piece);
    unsafe { T::load_pair(ip.add(a), ip.add(b), piece) }
}

/// Stores the first `len` bytes of `chunk` at `dst`.
#[inline(always)]
const unsafe fn store_prefix<T: Copy>(chunk: T, dst: *mut u8, len: usize) {
    unsafe {
        (&chunk as *const T)
            .cast::<u8>()
            .copy_to_nonoverlapping(dst, len)
    }
}
