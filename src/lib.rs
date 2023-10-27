//! [![github]](https://github.com/danipopes/const-hex)&ensp;[![crates-io]](https://crates.io/crates/const-hex)&ensp;[![docs-rs]](https://docs.rs/const-hex)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! This crate provides a fast conversion of byte arrays to hexadecimal strings,
//! both at compile time, and at run time.
//!
//! Extends the [`hex`] crate's implementation with [const-eval](const_encode), a
//! [const-generics formatting buffer](Buffer), similar to [`itoa`]'s, and more.
//!
//! _Version requirement: rustc 1.64+_
//!
//! [`itoa`]: https://docs.rs/itoa/latest/itoa/struct.Buffer.html
#![cfg_attr(not(feature = "hex"), doc = "[`hex`]: https://docs.rs/hex")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(feature = "nightly", feature(core_intrinsics, inline_const))]
#![cfg_attr(feature = "portable-simd", feature(portable_simd))]
#![warn(
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    unsafe_op_in_unsafe_fn,
    clippy::missing_const_for_fn,
    clippy::missing_inline_in_public_items,
    clippy::all,
    rustdoc::all
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![deny(unused_must_use, rust_2018_idioms)]
#![allow(
    clippy::cast_lossless,
    clippy::inline_always,
    clippy::let_unit_value,
    clippy::must_use_candidate,
    clippy::wildcard_imports,
    unsafe_op_in_unsafe_fn,
    unused_unsafe
)]

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;

use cfg_if::cfg_if;

#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

// `cpufeatures` may be unused when `force-generic` is enabled.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use cpufeatures as _;

// The main implementation functions.
cfg_if! {
    if #[cfg(feature = "force-generic")] {
        use generic as imp;
    } else if #[cfg(feature = "portable-simd")] {
        mod portable_simd;
        use portable_simd as imp;
    } else if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
        mod x86;
        use x86 as imp;
    } else if #[cfg(all(target_arch = "aarch64", target_feature = "neon"))] {
        mod aarch64;
        use aarch64 as imp;
    } else {
        use generic as imp;
    }
}

// If the `hex` feature is enabled, re-export the `hex` crate's traits.
// Otherwise, use our own with the more optimized implementation.
cfg_if! {
    if #[cfg(feature = "hex")] {
        pub use hex;

        #[doc(inline)]
        pub use hex::{FromHex, FromHexError, ToHex};
    } else {
        mod error;
        pub use error::FromHexError;

        mod traits;
        #[allow(deprecated)]
        pub use traits::{FromHex, ToHex};
    }
}

// Support for nightly features.
cfg_if! {
    if #[cfg(feature = "nightly")] {
        // Branch prediction hints.
        #[allow(unused_imports)]
        use core::intrinsics::{likely, unlikely};

        // `inline_const`: [#76001](https://github.com/rust-lang/rust/issues/76001)
        macro_rules! maybe_const_assert {
            ($($tt:tt)*) => {
                const { assert!($($tt)*) }
            };
        }
    } else {
        // On stable we can use #[cold] to get a equivalent effect: this attribute
        // suggests that the function is unlikely to be called
        #[inline(always)]
        #[cold]
        #[allow(clippy::missing_const_for_fn)]
        fn cold() {}

        #[inline(always)]
        #[allow(dead_code)]
        #[allow(clippy::missing_const_for_fn)]
        fn likely(b: bool) -> bool {
            if !b {
                cold();
            }
            b
        }

        #[inline(always)]
        #[allow(clippy::missing_const_for_fn)]
        fn unlikely(b: bool) -> bool {
            if b {
                cold();
            }
            b
        }

        macro_rules! maybe_const_assert {
            ($($tt:tt)*) => {
                assert!($($tt)*)
            };
        }
    }
}

// Serde support.
cfg_if! {
    if #[cfg(feature = "serde")] {
        pub mod serde;

        #[doc(no_inline)]
        pub use self::serde::deserialize;
        #[cfg(feature = "alloc")]
        #[doc(no_inline)]
        pub use self::serde::{serialize, serialize_upper};
    }
}

mod buffer;
pub use buffer::Buffer;

/// The table of lowercase characters used for hex encoding.
pub const HEX_CHARS_LOWER: &[u8; 16] = b"0123456789abcdef";

/// The table of uppercase characters used for hex encoding.
pub const HEX_CHARS_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// The lookup table of hex byte to value, used for hex decoding.
///
/// [`NIL`] is used for invalid values.
pub const HEX_DECODE_LUT: &[u8; 256] = &make_decode_lut();

/// Represents an invalid value in the [`HEX_DECODE_LUT`] table.
pub const NIL: u8 = u8::MAX;

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
pub const fn const_encode<const N: usize, const PREFIX: bool>(
    input: &[u8; N],
) -> Buffer<N, PREFIX> {
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
#[inline]
pub fn encode_to_slice<T: AsRef<[u8]>>(input: T, output: &mut [u8]) -> Result<(), FromHexError> {
    encode_to_slice_inner::<false>(input.as_ref(), output)
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
#[inline]
pub fn encode_to_slice_upper<T: AsRef<[u8]>>(
    input: T,
    output: &mut [u8],
) -> Result<(), FromHexError> {
    encode_to_slice_inner::<true>(input.as_ref(), output)
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
#[inline]
pub fn encode<T: AsRef<[u8]>>(data: T) -> String {
    encode_inner::<false, false>(data.as_ref())
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
#[inline]
pub fn encode_upper<T: AsRef<[u8]>>(data: T) -> String {
    encode_inner::<true, false>(data.as_ref())
}

/// Encodes `data` as a prefixed hex string using lowercase characters.
///
/// See [`encode()`] for more details.
///
/// # Examples
///
/// ```
/// assert_eq!(const_hex::encode_prefixed("Hello world!"), "0x48656c6c6f20776f726c6421");
/// assert_eq!(const_hex::encode_prefixed([1, 2, 3, 15, 16]), "0x0102030f10");
/// ```
#[cfg(feature = "alloc")]
#[inline]
pub fn encode_prefixed<T: AsRef<[u8]>>(data: T) -> String {
    encode_inner::<false, true>(data.as_ref())
}

/// Encodes `data` as a prefixed hex string using uppercase characters.
///
/// See [`encode_upper()`] for more details.
///
/// # Examples
///
/// ```
/// assert_eq!(const_hex::encode_upper_prefixed("Hello world!"), "0x48656C6C6F20776F726C6421");
/// assert_eq!(const_hex::encode_upper_prefixed([1, 2, 3, 15, 16]), "0x0102030F10");
/// ```
#[cfg(feature = "alloc")]
#[inline]
pub fn encode_upper_prefixed<T: AsRef<[u8]>>(data: T) -> String {
    encode_inner::<true, true>(data.as_ref())
}

/// Decodes a hex string into raw bytes.
///
/// Both, upper and lower case characters are valid in the input string and can
/// even be mixed (e.g. `f9b4ca`, `F9B4CA` and `f9B4Ca` are all valid strings).
///
/// Strips the `0x` prefix if present.
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
/// assert_eq!(
///     const_hex::decode("0x48656c6c6f20776f726c6421"),
///     Ok("Hello world!".to_owned().into_bytes())
/// );
///
/// assert_eq!(const_hex::decode("123"), Err(const_hex::FromHexError::OddLength));
/// assert!(const_hex::decode("foo").is_err());
/// ```
#[cfg(feature = "alloc")]
#[inline]
pub fn decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, FromHexError> {
    fn decode_inner(input: &[u8]) -> Result<Vec<u8>, FromHexError> {
        if unlikely(input.len() % 2 != 0) {
            return Err(FromHexError::OddLength);
        }
        let input = strip_prefix(input);

        // Do not initialize memory since it will be entirely overwritten.
        let len = input.len() / 2;
        let mut output = Vec::with_capacity(len);
        // SAFETY: The entire vec is never read from, and gets dropped if decoding fails.
        #[allow(clippy::uninit_vec)]
        unsafe {
            output.set_len(len);
        }

        // SAFETY: Lengths are checked above.
        unsafe { decode_real(input, &mut output) }.map(|()| output)
    }

    decode_inner(input.as_ref())
}

/// Decode a hex string into a mutable bytes slice.
///
/// Both, upper and lower case characters are valid in the input string and can
/// even be mixed (e.g. `f9b4ca`, `F9B4CA` and `f9B4Ca` are all valid strings).
///
/// Strips the `0x` prefix if present.
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
/// const_hex::decode_to_slice("6b697769", &mut bytes).unwrap();
/// assert_eq!(&bytes, b"kiwi");
///
/// const_hex::decode_to_slice("0x6b697769", &mut bytes).unwrap();
/// assert_eq!(&bytes, b"kiwi");
/// ```
#[inline]
pub fn decode_to_slice<T: AsRef<[u8]>>(input: T, output: &mut [u8]) -> Result<(), FromHexError> {
    fn decode_to_slice_inner(input: &[u8], output: &mut [u8]) -> Result<(), FromHexError> {
        if unlikely(input.len() % 2 != 0) {
            return Err(FromHexError::OddLength);
        }
        let input = strip_prefix(input);
        if unlikely(output.len() != input.len() / 2) {
            return Err(FromHexError::InvalidStringLength);
        }
        // SAFETY: Lengths are checked above.
        unsafe { decode_real(input, output) }
    }

    decode_to_slice_inner(input.as_ref(), output)
}

#[cfg(feature = "alloc")]
fn encode_inner<const UPPER: bool, const PREFIX: bool>(data: &[u8]) -> String {
    let mut buf = vec![0; (PREFIX as usize + data.len()) * 2];
    let output = if PREFIX {
        buf[0] = b'0';
        buf[1] = b'x';
        &mut buf[2..]
    } else {
        &mut buf[..]
    };
    // SAFETY: `output` is long enough (input.len() * 2).
    unsafe { imp::encode::<UPPER>(data, output.as_mut_ptr()) };
    // SAFETY: We only write only ASCII bytes.
    unsafe { String::from_utf8_unchecked(buf) }
}

fn encode_to_slice_inner<const UPPER: bool>(
    input: &[u8],
    output: &mut [u8],
) -> Result<(), FromHexError> {
    if unlikely(output.len() != 2 * input.len()) {
        return Err(FromHexError::InvalidStringLength);
    }
    // SAFETY: Lengths are checked above.
    unsafe { imp::encode::<UPPER>(input, output.as_mut_ptr()) };
    Ok(())
}

unsafe fn decode_real(input: &[u8], output: &mut [u8]) -> Result<(), FromHexError> {
    if imp::USE_CHECK_FN {
        // check then decode
        if imp::check(input) {
            unsafe { imp::decode_unchecked(input, output) };
            return Ok(());
        }
    } else {
        // check and decode at the same time
        if unsafe { imp::decode_checked(input, output) } {
            return Ok(());
        }
    }

    Err(unsafe { invalid_hex_error(input) })
}

mod generic {
    use super::*;

    /// Set to `true` to use `check` + `decode_unchecked`. Otherwise uses `decode_checked`.
    ///
    /// This should be set to `false` if `check` is not specialized.
    #[allow(dead_code)]
    pub(super) const USE_CHECK_FN: bool = false;

    /// Default encoding function.
    ///
    /// # Safety
    ///
    /// `output` must be a valid pointer to at least `2 * input.len()` bytes.
    pub(super) unsafe fn encode<const UPPER: bool>(input: &[u8], output: *mut u8) {
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
    pub(super) fn check(input: &[u8]) -> bool {
        input
            .iter()
            .all(|byte| HEX_DECODE_LUT[*byte as usize] != NIL)
    }

    /// Default unchecked decoding function.
    ///
    /// # Safety
    ///
    /// Assumes `output.len() == input.len() / 2`.
    pub(super) unsafe fn decode_checked(input: &[u8], output: &mut [u8]) -> bool {
        unsafe { decode_maybe_check::<true>(input, output) }
    }

    /// Default unchecked decoding function.
    ///
    /// # Safety
    ///
    /// Assumes `output.len() == input.len() / 2` and that the input is valid hex.
    pub(super) unsafe fn decode_unchecked(input: &[u8], output: &mut [u8]) {
        let r = unsafe { decode_maybe_check::<false>(input, output) };
        debug_assert!(r);
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
                } else {
                    debug_assert_ne!($var, NIL);
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
}

#[inline]
const fn byte2hex<const UPPER: bool>(byte: u8) -> (u8, u8) {
    let table = get_chars_table::<UPPER>();
    let high = table[((byte & 0xf0) >> 4) as usize];
    let low = table[(byte & 0x0f) as usize];
    (high, low)
}

#[inline]
fn strip_prefix(bytes: &[u8]) -> &[u8] {
    if bytes.starts_with(b"0x") {
        unsafe { bytes.get_unchecked(2..) }
    } else {
        bytes
    }
}

#[inline(always)]
const fn get_chars_table<const UPPER: bool>() -> &'static [u8; 16] {
    if UPPER {
        HEX_CHARS_UPPER
    } else {
        HEX_CHARS_LOWER
    }
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
            _ => NIL,
        };
        if i == NIL {
            break;
        }
        i += 1;
    }
    lut
}

/// Creates an invalid hex error from the input.
///
/// # Safety
///
/// Assumes `input` contains at least one invalid character.
#[cold]
#[cfg_attr(debug_assertions, track_caller)]
unsafe fn invalid_hex_error(input: &[u8]) -> FromHexError {
    let index = input
        .iter()
        .position(|byte| HEX_DECODE_LUT[*byte as usize] == NIL);
    debug_assert!(index.is_some(), "input was valid but `check` failed");
    let index = index.unwrap_unchecked();
    FromHexError::InvalidHexCharacter {
        c: input[index] as char,
        index,
    }
}
