//! Hex encoding with [`serde`](serde_core).
//!
//! # Examples
//!
//! ```
//! # #[cfg(feature = "alloc")] {
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct Foo {
//!     #[serde(with = "const_hex")]
//!     bar: Vec<u8>,
//! }
//! # }
//! ```

use crate::FromHex;
use core::fmt;
use core::marker::PhantomData;
use serde_core::de::{Error, Visitor};
use serde_core::Deserializer;

/// Serializes `data` as hex string using lowercase characters with a `0x` prefix.
///
/// Lowercase characters are used (e.g. `f9b4ca`). The resulting string's length
/// is always even, each byte in data is always encoded using two hex digits.
/// Thus, the resulting string contains exactly twice as many bytes as the input
/// data plus two (for the prefix).
#[inline]
pub fn serialize<S, T>(data: T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde_core::Serializer,
    T: AsRef<[u8]>,
{
    serialize_inner::<S, false, true>(data.as_ref(), serializer)
}

/// Serializes `data` as hex string using uppercase characters.
///
/// Apart from the characters' casing, this works exactly like [`serialize`].
#[inline]
pub fn serialize_upper<S, T>(data: T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde_core::Serializer,
    T: AsRef<[u8]>,
{
    serialize_inner::<S, true, true>(data.as_ref(), serializer)
}

/// Inputs up to this many bytes are encoded into a stack buffer.
const STACK_LEN: usize = 128;

/// Encodes into a single buffer and emits it with one `serialize_str` call.
///
/// `collect_str` would instead run the `Display` impl against the serializer's `fmt::Write`
/// shim, which for `serde_json` means one string-escape scan and one `write_all` per SIMD
/// chunk (and one per nibble for the scalar tail), i.e. tens of calls per value.
fn serialize_inner<S, const UPPER: bool, const PREFIX: bool>(
    data: &[u8],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde_core::Serializer,
{
    if data.len() <= STACK_LEN {
        let mut buf = crate::impl_core::uninit_array::<u8, { STACK_LEN * 2 + 2 }>();
        let prefix_len = PREFIX as usize * 2;
        let len = prefix_len + data.len() * 2;
        if PREFIX {
            buf[0].write(b'0');
            buf[1].write(b'x');
        }
        // SAFETY: `buf[prefix_len..len]` is exactly `data.len() * 2` bytes long.
        unsafe { crate::imp::encode::<UPPER>(data, &mut buf[prefix_len..len]) };
        // SAFETY: `buf[..len]` has been fully initialized above, and only ASCII
        // characters, which are valid UTF-8, have been written.
        let s = unsafe {
            core::str::from_utf8_unchecked(crate::impl_core::slice_assume_init(&buf[..len]))
        };
        serializer.serialize_str(s)
    } else {
        #[cfg(feature = "alloc")]
        {
            serializer.serialize_str(&crate::encode_inner::<UPPER, PREFIX>(data))
        }
        #[cfg(not(feature = "alloc"))]
        {
            let display = crate::display(data);
            match (UPPER, PREFIX) {
                (false, false) => serializer.collect_str(&format_args!("{display:x}")),
                (false, true) => serializer.collect_str(&format_args!("{display:#x}")),
                (true, false) => serializer.collect_str(&format_args!("{display:X}")),
                (true, true) => serializer.collect_str(&format_args!("{display:#X}")),
            }
        }
    }
}

/// Deserializes a hex string into raw bytes.
///
/// Both, upper and lower case characters are valid in the input string and can
/// even be mixed (e.g. `f9b4ca`, `F9B4CA` and `f9B4Ca` are all valid strings).
#[inline]
pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromHex,
    <T as FromHex>::Error: fmt::Display,
{
    struct HexStrVisitor<T>(PhantomData<T>);

    impl<T> Visitor<'_> for HexStrVisitor<T>
    where
        T: FromHex,
        <T as FromHex>::Error: fmt::Display,
    {
        type Value = T;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a hex encoded string")
        }

        fn visit_bytes<E: Error>(self, data: &[u8]) -> Result<Self::Value, E> {
            FromHex::from_hex(data).map_err(Error::custom)
        }

        fn visit_str<E: Error>(self, data: &str) -> Result<Self::Value, E> {
            FromHex::from_hex(data.as_bytes()).map_err(Error::custom)
        }
    }

    deserializer.deserialize_str(HexStrVisitor(PhantomData))
}

/// Hex encoding with [`serde`](serde_core).
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "alloc")] {
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Foo {
///     #[serde(with = "const_hex::serde::no_prefix")]
///     bar: Vec<u8>,
/// }
/// # }
/// ```
pub mod no_prefix {
    /// Serializes `data` as hex string using lowercase characters.
    ///
    /// Lowercase characters are used (e.g. `f9b4ca`). The resulting string's length
    /// is always even, each byte in data is always encoded using two hex digits.
    /// Thus, the resulting string contains exactly twice as many bytes as the input
    /// data.
    #[inline]
    pub fn serialize<S, T>(data: T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
        T: AsRef<[u8]>,
    {
        super::serialize_inner::<S, false, false>(data.as_ref(), serializer)
    }

    /// Serializes `data` as hex string using uppercase characters.
    ///
    /// Apart from the characters' casing, this works exactly like [`serialize`].
    #[inline]
    pub fn serialize_upper<S, T>(data: T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
        T: AsRef<[u8]>,
    {
        super::serialize_inner::<S, true, false>(data.as_ref(), serializer)
    }

    pub use super::deserialize;
}
