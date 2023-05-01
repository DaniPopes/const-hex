//! [![github]](https://github.com/danipopes/const-hex)&ensp;[![crates-io]](https://crates.io/crates/const-hex)&ensp;[![docs-rs]](https://docs.rs/const-hex)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! This crate provides a fast conversion of byte arrays to hexadecimal strings.
//! The implementation comes mostly from, and extends, the [`hex`] crate, but
//! avoids the performance penalty of going through [`core::fmt::Formatter`] or any
//! heap allocation.
//!
//! _Version requirement: rustc 1.64+_

#![doc(html_root_url = "https://docs.rs/const-hex/1.0.0")]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::must_use_candidate)]

use core::slice;
use core::str;

#[cfg(feature = "hex")]
#[doc(inline)]
pub use hex::{decode, decode_to_slice, encode, encode_upper, FromHex, FromHexError, ToHex};

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
/// # Example
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
        unsafe { core::str::from_utf8_unchecked_mut(buf) }
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

#[cold]
#[inline(never)]
#[track_caller]
fn length_mismatch() -> ! {
    panic!("length mismatch");
}

#[inline]
const fn byte2hex(byte: u8, table: &[u8; 16]) -> (u8, u8) {
    let high = table[((byte & 0xf0) >> 4) as usize];
    let low = table[(byte & 0x0f) as usize];
    (high, low)
}

/// Encodes some bytes into a mutable slice of bytes.
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

/// Encodes some bytes into a mutable slice of bytes.
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

#[inline]
fn encode_to_slice_inner(
    input: &[u8],
    output: &mut [u8],
    table: &[u8; 16],
) -> Result<(), FromHexError> {
    if output.len() != input.len() * 2 {
        return Err(FromHexError::InvalidStringLength);
    }

    let mut c = 0;
    for byte in input.iter() {
        let (high, low) = byte2hex(*byte, table);
        output[c] = high;
        c = c.wrapping_add(1);
        output[c] = low;
        c = c.wrapping_add(1);
    }
    Ok(())
}

/// Encodes some bytes into a [`Buffer`].
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
