//! [![github]](https://github.com/danipopes/const-hex)&ensp;[![crates-io]](https://crates.io/crates/const-hex)&ensp;[![docs-rs]](https://docs.rs/const-hex)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! This crate provides a fast conversion of byte arrays to hexadecimal strings,
//! both at compile time, and at run time.
//!
//! Extends the [`hex`] crate's implementation with [const-eval], a
//! [const-generics formatting buffer][Buffer], similar to [`itoa`]'s, and more.
//!
//! _Version requirement: rustc 1.64+_
//!
//! [const-eval]: const_encode
//! [`itoa`]: https://docs.rs/itoa/latest/itoa/struct.Buffer.html
#![cfg_attr(not(feature = "hex"), doc = "[`hex`]: https://docs.rs/hex")]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::must_use_candidate, clippy::wildcard_imports)]

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;

use cfg_if::cfg_if;
use core::slice;
use core::str;

#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

// The main encoding function. Assumes `output.len() == 2 * input.len()`.
cfg_if! {
    if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
        mod x86;
        use x86::_encode;
    } else {
        use encode_default as _encode;
    }
}

// FIXME: x86 decode implementation.
use decode_default as _decode;

cfg_if! {
    if #[cfg(feature = "hex")] {
        #[doc(inline)]
        pub use hex::{FromHex, FromHexError, ToHex};

        #[cfg(feature = "serde")]
        #[doc(inline)]
        pub use hex::serde;
    } else {
        mod error;
        pub use error::FromHexError;

        mod traits;
        pub use traits::{FromHex, ToHex};

        #[cfg(feature = "serde")]
        pub mod serde;
    }
}

#[cfg(feature = "serde")]
#[doc(no_inline)]
pub use self::serde::deserialize;
#[cfg(all(feature = "serde", feature = "alloc"))]
#[doc(no_inline)]
pub use self::serde::{serialize, serialize_upper};

/// The table of lowercase characters used for hex encoding.
pub const HEX_CHARS_LOWER: &[u8; 16] = b"0123456789abcdef";

/// The table of uppercase characters used for hex encoding.
pub const HEX_CHARS_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// The lookup table of hex byte to value, used for hex decoding.
///
/// [`u8::MAX`] is used for invalid values.
pub const HEX_DECODE_LUT: &[u8; 256] = &make_decode_lut();

/// A correctly sized stack allocation for the formatted bytes to be written
/// into.
///
/// # Examples
///
/// ```
/// let mut buffer = const_hex::Buffer::new();
/// let printed = buffer.format(b"1234");
/// assert_eq!(printed, "31323334");
/// ```
#[must_use]
pub struct Buffer<const N: usize> {
    /// Workaround for not being able to do operations with constants: `[u8; N * 2]`
    bytes: [u16; N],
}

impl<const N: usize> Default for Buffer<N> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Clone for Buffer<N> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl<const N: usize> Buffer<N> {
    /// This is a cheap operation; you don't need to worry about reusing buffers
    /// for efficiency.
    #[inline]
    pub const fn new() -> Self {
        Self { bytes: [0; N] }
    }

    /// Clears the buffer.
    #[inline]
    pub fn clear(&mut self) {
        self.bytes = [0; N];
    }

    /// Consumes and clears the buffer.
    #[inline]
    pub const fn cleared(mut self) -> Self {
        self.bytes = [0; N];
        self
    }

    /// Print an array of bytes into this buffer.
    #[inline]
    pub const fn const_format(self, array: &[u8; N]) -> Self {
        self.const_format_inner(array, HEX_CHARS_LOWER)
    }

    /// Print an array of bytes into this buffer.
    #[inline]
    pub const fn const_format_upper(self, array: &[u8; N]) -> Self {
        self.const_format_inner(array, HEX_CHARS_UPPER)
    }

    /// Same as [`encode_to_slice_inner`], but const-stable.
    const fn const_format_inner(mut self, array: &[u8; N], table: &[u8; 16]) -> Self {
        let mut i = 0;
        while i < N {
            let (high, low) = byte2hex(array[i], table);
            self.bytes[i] = u16::from_le_bytes([high, low]);
            i += 1;
        }
        self
    }

    /// Print an array of bytes into this buffer and return a reference to its
    /// *lower* hex string representation within the buffer.
    #[inline]
    pub fn format(&mut self, array: &[u8; N]) -> &mut str {
        // length of array is guaranteed to be N.
        self.format_inner(array, HEX_CHARS_LOWER)
    }

    /// Print an array of bytes into this buffer and return a reference to its
    /// *upper* hex string representation within the buffer.
    #[inline]
    pub fn format_upper(&mut self, array: &[u8; N]) -> &mut str {
        // length of array is guaranteed to be N.
        self.format_inner(array, HEX_CHARS_UPPER)
    }

    /// Print a slice of bytes into this buffer and return a reference to its
    /// *lower* hex string representation within the buffer.
    ///
    /// # Panics
    ///
    /// If the slice is not exactly `N` bytes long.
    #[track_caller]
    pub fn format_slice<T: AsRef<[u8]>>(&mut self, slice: T) -> &mut str {
        self.format_slice_inner(slice.as_ref(), HEX_CHARS_LOWER)
    }

    /// Print a slice of bytes into this buffer and return a reference to its
    /// *upper* hex string representation within the buffer.
    ///
    /// # Panics
    ///
    /// If the slice is not exactly `N` bytes long.
    #[track_caller]
    pub fn format_slice_upper<T: AsRef<[u8]>>(&mut self, slice: T) -> &mut str {
        self.format_slice_inner(slice.as_ref(), HEX_CHARS_UPPER)
    }

    // Checks length
    #[track_caller]
    #[inline]
    fn format_slice_inner(&mut self, slice: &[u8], table: &[u8; 16]) -> &mut str {
        assert_eq!(slice.len(), N, "length mismatch");
        self.format_inner(slice, table)
    }

    // Doesn't check length
    #[inline]
    fn format_inner(&mut self, input: &[u8], table: &[u8; 16]) -> &mut str {
        let buf = self.as_mut_bytes();
        // SAFETY: Length was checked previously.
        unsafe { _encode(input, buf, table) };
        // SAFETY: We only write only ASCII bytes.
        unsafe { str::from_utf8_unchecked_mut(buf) }
    }

    /// Returns a reference to the underlying bytes casted to a string slice.
    ///
    /// Note that this contains only null ('\0') bytes before any formatting
    /// is done.
    #[inline]
    pub const fn as_str(&self) -> &str {
        // SAFETY: The buffer always contains valid UTF-8.
        let bytes = self.as_bytes();
        unsafe { str::from_utf8_unchecked(bytes) }
    }

    /// Returns a reference to the underlying bytes casted to a string slice.
    ///
    /// Note that this contains only null ('\0') bytes before any formatting
    /// is done.
    #[inline]
    pub fn as_mut_str(&mut self) -> &mut str {
        // SAFETY: The buffer always contains valid UTF-8.
        let bytes = self.as_mut_bytes();
        unsafe { str::from_utf8_unchecked_mut(bytes) }
    }

    /// Returns a reference to the underlying byte slice.
    ///
    /// Note that this contains only null ('\0') bytes before any formatting
    /// is done.
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] {
        // SAFETY: [u16; N] is layout-compatible with [u8; N * 2].
        let ptr = self.bytes.as_ptr().cast::<u8>();
        unsafe { slice::from_raw_parts(ptr, N * 2) }
    }

    /// Returns a mutable reference to the underlying byte slice.
    ///
    /// Note that this contains only null ('\0') bytes before any formatting
    /// is done.
    ///
    /// Not public API because other methods rely on the internal buffer always
    /// being valid UTF-8.
    #[inline]
    fn as_mut_bytes(&mut self) -> &mut [u8] {
        // SAFETY: [u16; N] is layout-compatible with [u8; N * 2].
        let ptr = self.bytes.as_mut_ptr().cast::<u8>();
        unsafe { slice::from_raw_parts_mut(ptr, N * 2) }
    }
}

/// Encodes `input` as a hex string into a [`Buffer`].
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), const_hex::FromHexError> {
/// const BUFFER: const_hex::Buffer<4> = const_hex::const_encode(b"kiwi");
/// assert_eq!(BUFFER.as_str(), "6b697769");
/// # Ok(())
/// # }
/// ```
#[inline]
pub const fn const_encode<const N: usize>(input: &[u8; N]) -> Buffer<N> {
    Buffer::new().const_format(input)
}

/// Encodes `input` as a hex string using lowercase characters into a mutable
/// slice of bytes `output`.
///
/// # Errors
///
/// If the output buffer is not exactly `input.len() * 2` bytes long.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), const_hex::FromHexError> {
/// let mut bytes = [0u8; 4 * 2];
/// const_hex::encode_to_slice(b"kiwi", &mut bytes)?;
/// assert_eq!(&bytes, b"6b697769");
/// # Ok(())
/// # }
/// ```
pub fn encode_to_slice<T: AsRef<[u8]>>(input: T, output: &mut [u8]) -> Result<(), FromHexError> {
    encode_to_slice_inner(input.as_ref(), output, HEX_CHARS_LOWER)
}

/// Encodes `input` as a hex string using uppercase characters into a mutable
/// slice of bytes `output`.
///
/// # Errors
///
/// If the output buffer is not exactly `input.len() * 2` bytes long.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), const_hex::FromHexError> {
/// let mut bytes = [0u8; 4 * 2];
/// const_hex::encode_to_slice_upper(b"kiwi", &mut bytes)?;
/// assert_eq!(&bytes, b"6B697769");
/// # Ok(())
/// # }
/// ```
pub fn encode_to_slice_upper<T: AsRef<[u8]>>(
    input: T,
    output: &mut [u8],
) -> Result<(), FromHexError> {
    encode_to_slice_inner(input.as_ref(), output, HEX_CHARS_UPPER)
}

/// Encodes `data` as a hex string using lowercase characters.
///
/// Lowercase characters are used (e.g. `f9b4ca`). The resulting string's
/// length is always even, each byte in `data` is always encoded using two hex
/// digits. Thus, the resulting string contains exactly twice as many bytes as
/// the input data.
///
/// # Examples
///
/// ```
/// assert_eq!(const_hex::encode("Hello world!"), "48656c6c6f20776f726c6421");
/// assert_eq!(const_hex::encode([1, 2, 3, 15, 16]), "0102030f10");
/// ```
#[cfg(feature = "alloc")]
pub fn encode<T: AsRef<[u8]>>(data: T) -> String {
    encode_inner(data.as_ref(), HEX_CHARS_LOWER)
}

/// Encodes `data` as a hex string using uppercase characters.
///
/// Apart from the characters' casing, this works exactly like `encode()`.
///
/// # Examples
///
/// ```
/// assert_eq!(const_hex::encode_upper("Hello world!"), "48656C6C6F20776F726C6421");
/// assert_eq!(const_hex::encode_upper([1, 2, 3, 15, 16]), "0102030F10");
/// ```
#[cfg(feature = "alloc")]
pub fn encode_upper<T: AsRef<[u8]>>(data: T) -> String {
    encode_inner(data.as_ref(), HEX_CHARS_UPPER)
}

/// Decodes a hex string into raw bytes.
///
/// Both, upper and lower case characters are valid in the input string and can
/// even be mixed (e.g. `f9b4ca`, `F9B4CA` and `f9B4Ca` are all valid strings).
///
/// # Errors
///
/// This function returns an error if the input is not an even number of
/// characters long or contains invalid hex characters.
///
/// # Example
///
/// ```
/// assert_eq!(
///     const_hex::decode("48656c6c6f20776f726c6421"),
///     Ok("Hello world!".to_owned().into_bytes())
/// );
///
/// assert_eq!(const_hex::decode("123"), Err(const_hex::FromHexError::OddLength));
/// assert!(const_hex::decode("foo").is_err());
/// ```
#[cfg(feature = "alloc")]
pub fn decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, FromHexError> {
    fn decode_inner(input: &[u8]) -> Result<Vec<u8>, FromHexError> {
        if input.len() % 2 != 0 {
            return Err(FromHexError::OddLength);
        }
        let mut output = vec![0u8; input.len() / 2];
        // SAFETY: Lengths are checked above.
        unsafe { _decode(input, &mut output)? };
        Ok(output)
    }

    decode_inner(input.as_ref())
}

/// Decode a hex string into a mutable bytes slice.
///
/// Both, upper and lower case characters are valid in the input string and can
/// even be mixed (e.g. `f9b4ca`, `F9B4CA` and `f9B4Ca` are all valid strings).
///
/// # Errors
///
/// This function returns an error if the input is not an even number of
/// characters long or contains invalid hex characters, or if the output slice
/// is not exactly half the length of the input.
///
/// # Example
///
/// ```
/// let mut bytes = [0u8; 4];
/// assert_eq!(const_hex::decode_to_slice("6b697769", &mut bytes as &mut [u8]), Ok(()));
/// assert_eq!(&bytes, b"kiwi");
/// ```
pub fn decode_to_slice<T: AsRef<[u8]>>(input: T, output: &mut [u8]) -> Result<(), FromHexError> {
    fn decode_to_slice_inner(input: &[u8], output: &mut [u8]) -> Result<(), FromHexError> {
        if input.len() % 2 != 0 {
            return Err(FromHexError::OddLength);
        }
        if output.len() != input.len() / 2 {
            return Err(FromHexError::InvalidStringLength);
        }
        // SAFETY: Lengths are checked above.
        unsafe { _decode(input, output) }
    }

    decode_to_slice_inner(input.as_ref(), output)
}

#[cfg(feature = "alloc")]
#[inline]
fn encode_inner(data: &[u8], table: &[u8; 16]) -> String {
    let mut output = vec![0; data.len() * 2];
    // SAFETY: `output` is long enough (input.len() * 2).
    unsafe { _encode(data, &mut output, table) };
    // SAFETY: We only write only ASCII bytes.
    unsafe { String::from_utf8_unchecked(output) }
}

#[inline]
fn encode_to_slice_inner(
    input: &[u8],
    output: &mut [u8],
    table: &[u8; 16],
) -> Result<(), FromHexError> {
    if output.len() != 2 * input.len() {
        return Err(FromHexError::InvalidStringLength);
    }
    // SAFETY: Lengths are checked above.
    unsafe { _encode(input, output, table) };
    Ok(())
}

/// Default encoding function.
///
/// # Safety
///
/// Assumes `output.len() == 2 * input.len()`.
unsafe fn encode_default(input: &[u8], output: &mut [u8], table: &[u8; 16]) {
    debug_assert_eq!(output.len(), 2 * input.len());
    let mut i = 0;
    for byte in input {
        let (high, low) = byte2hex(*byte, table);
        *output.get_unchecked_mut(i) = high;
        i = i.checked_add(1).unwrap_unchecked();
        *output.get_unchecked_mut(i) = low;
        i = i.checked_add(1).unwrap_unchecked();
    }
}

/// Default decoding function.
///
/// # Safety
///
/// Assumes `output.len() == input.len() / 2`.
unsafe fn decode_default(input: &[u8], output: &mut [u8]) -> Result<(), FromHexError> {
    macro_rules! next {
        ($var:ident, $i:expr) => {
            let hex = *input.get_unchecked($i);
            let $var = HEX_DECODE_LUT[hex as usize];
            if $var == u8::MAX {
                return Err(FromHexError::InvalidHexCharacter {
                    c: hex as char,
                    index: $i,
                });
            }
        };
    }

    debug_assert_eq!(output.len(), input.len() / 2);
    for (i, byte) in output.iter_mut().enumerate() {
        next!(high, i * 2);
        next!(low, i * 2 + 1);
        *byte = high << 4 | low;
    }
    Ok(())
}

#[inline]
const fn byte2hex(byte: u8, table: &[u8; 16]) -> (u8, u8) {
    let high = table[((byte & 0xf0) >> 4) as usize];
    let low = table[(byte & 0x0f) as usize];
    (high, low)
}

const fn make_decode_lut() -> [u8; 256] {
    let mut lut = [0; 256];
    let mut i = 0u8;
    loop {
        lut[i as usize] = match i {
            b'0'..=b'9' => i - b'0',
            b'A'..=b'F' => i - b'A' + 10,
            b'a'..=b'f' => i - b'a' + 10,
            // use max value for invalid characters
            _ => u8::MAX,
        };
        if i == u8::MAX {
            break;
        }
        i += 1;
    }
    lut
}
