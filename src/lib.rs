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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::must_use_candidate, clippy::wildcard_imports)]

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;

cfg_if::cfg_if! {
    if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
        mod x86;
        use x86::_encode;
    } else {
        use encode_default as _encode;
    }
}

use core::slice;
use core::str;

#[cfg(feature = "alloc")]
use alloc::string::String;

#[cfg(feature = "hex")]
#[doc(inline)]
pub use hex::{decode_to_slice, FromHex, FromHexError, ToHex};

#[cfg(all(feature = "hex", feature = "alloc"))]
#[doc(inline)]
pub use hex::decode;

#[cfg(not(feature = "hex"))]
#[doc(hidden)]
mod error;
#[cfg(not(feature = "hex"))]
#[doc(inline)]
pub use error::FromHexError;

/// The table of lowercase characters used for hex encoding.
pub const HEX_CHARS_LOWER: &[u8; 16] = b"0123456789abcdef";

/// The table of uppercase characters used for hex encoding.
pub const HEX_CHARS_UPPER: &[u8; 16] = b"0123456789ABCDEF";

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
    pub fn clear(&mut self) {
        self.bytes = [0; N];
    }

    /// Consumes and clears the buffer.
    pub const fn cleared(mut self) -> Self {
        self.bytes = [0; N];
        self
    }

    /// Print an array of bytes into this buffer.
    pub const fn const_format(self, array: &[u8; N]) -> Self {
        self.const_format_inner(array, HEX_CHARS_LOWER)
    }

    /// Print an array of bytes into this buffer.
    pub const fn const_format_upper(self, array: &[u8; N]) -> Self {
        self.const_format_inner(array, HEX_CHARS_UPPER)
    }

    /// Same as [`encode_to_slice_inner`], but const-stable.
    const fn const_format_inner(mut self, array: &[u8; N], table: &[u8; 16]) -> Self {
        let mut i = 0;
        while i < N {
            let (high, low) = byte2hex(array[i], table);
            self.bytes[i] = u16::from_le_bytes([high, low]);
            i = i.wrapping_add(1);
        }
        self
    }

    /// Print an array of bytes into this buffer and return a reference to its
    /// *lower* hex string representation within the buffer.
    pub fn format(&mut self, array: &[u8; N]) -> &mut str {
        // length of array is guaranteed to be N.
        self.format_inner(array, HEX_CHARS_LOWER)
    }

    /// Print an array of bytes into this buffer and return a reference to its
    /// *upper* hex string representation within the buffer.
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
        if slice.len() != N {
            length_mismatch();
        }
        self.format_inner(slice, table)
    }

    // Doesn't check length
    fn format_inner(&mut self, input: &[u8], table: &[u8; 16]) -> &mut str {
        let buf = self.as_mut_bytes();
        // SAFETY: Length was checked previously.
        unsafe { encode_to_slice_inner(input, buf, table).unwrap_unchecked() };
        // SAFETY: `encode_to_slice` writes only ASCII bytes.
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

#[cfg(feature = "alloc")]
fn encode_inner(data: &[u8], table: &[u8; 16]) -> String {
    let mut output = vec![0u8; data.len() * 2];
    // SAFETY: `output` is long enough (input.len() * 2).
    unsafe { encode_to_slice_inner(data, &mut output, table).unwrap_unchecked() };
    // SAFETY: `encode_to_slice` writes only ASCII bytes.
    unsafe { String::from_utf8_unchecked(output) }
}

/// The main encoding function.
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

/// # Safety
///
/// `output.len() == 2 * input.len()`
#[inline]
unsafe fn encode_default(input: &[u8], output: &mut [u8], table: &[u8; 16]) {
    let mut c = 0;
    for byte in input.iter() {
        let (high, low) = byte2hex(*byte, table);
        *output.get_unchecked_mut(c) = high;
        c = c.wrapping_add(1);
        *output.get_unchecked_mut(c) = low;
        c = c.wrapping_add(1);
    }
}

#[inline]
const fn byte2hex(byte: u8, table: &[u8; 16]) -> (u8, u8) {
    let high = table[((byte & 0xf0) >> 4) as usize];
    let low = table[(byte & 0x0f) as usize];
    (high, low)
}

#[cold]
#[inline(never)]
#[track_caller]
fn length_mismatch() -> ! {
    panic!("length mismatch");
}
