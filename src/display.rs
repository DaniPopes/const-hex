use core::fmt;
use core::mem::MaybeUninit;

/// Returns a value that can be formatted using the [`fmt`] traits.
///
/// Supports [`fmt::LowerHex`], [`fmt::UpperHex`], and [`fmt::Display`]
/// (which is the same as [`fmt::LowerHex`]),
/// as well as using the alternate flag (`:#`) to write the hex prefix.
///
/// # Examples
///
/// ```
/// let bytes: &[u8] = &[0xde, 0xad, 0xbe, 0xef];
/// let displayed = const_hex::display(bytes);
/// let s = format!("{displayed} {displayed:#X}");
/// assert_eq!(s, "deadbeef 0xDEADBEEF");
/// ```
#[inline]
pub fn display<T: AsRef<[u8]>>(input: T) -> impl fmt::Display + fmt::LowerHex + fmt::UpperHex {
    Display(input)
}

struct Display<T: AsRef<[u8]>>(T);

impl<T: AsRef<[u8]>> fmt::Display for Display<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(self, f)
    }
}

impl<T: AsRef<[u8]>> fmt::LowerHex for Display<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex::<false>(self.0.as_ref(), f.alternate(), f)
    }
}

impl<T: AsRef<[u8]>> fmt::UpperHex for Display<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex::<true>(self.0.as_ref(), f.alternate(), f)
    }
}

/// Input bytes encoded per `write_str` call.
const CHUNK_LEN: usize = 1024;

/// Writes `input` as hex to `f`, with one `write_str` per chunk of `CHUNK_LEN` input bytes.
///
/// The `0x` prefix, if any, is written together with the first chunk.
pub(crate) fn write_hex<const UPPER: bool>(
    input: &[u8],
    prefix: bool,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if input.is_empty() {
        return if prefix { f.write_str("0x") } else { Ok(()) };
    }
    let mut buf = crate::impl_core::uninit_array::<u8, { 2 + CHUNK_LEN * 2 }>();
    for (i, chunk) in input.chunks(CHUNK_LEN).enumerate() {
        f.write_str(encode_str::<UPPER>(chunk, prefix && i == 0, &mut buf))?;
    }
    Ok(())
}

/// Encodes `input` as hex into `buf` and returns it as a string, with a `0x` prefix if `prefix`.
///
/// The prefix goes in `buf[0..2]` and the hex digits in `buf[2..]`, so `buf` must be at least
/// `2 + input.len() * 2` bytes long.
#[inline]
pub(crate) fn encode_str<'a, const UPPER: bool>(
    input: &[u8],
    prefix: bool,
    buf: &'a mut [MaybeUninit<u8>],
) -> &'a str {
    let end = 2 + input.len() * 2;
    let start = if prefix {
        buf[0].write(b'0');
        buf[1].write(b'x');
        0
    } else {
        2
    };
    // SAFETY: `buf[2..end]` is exactly `input.len() * 2` bytes long.
    unsafe { crate::encode_uninit::<UPPER>(input, &mut buf[2..end]) };
    // SAFETY: `buf[start..end]` is initialized: the prefix, if `prefix`, and the encoded input.
    // Only ASCII characters, which are valid UTF-8, have been written.
    unsafe { core::str::from_utf8_unchecked(crate::impl_core::slice_assume_init(&buf[start..end])) }
}
