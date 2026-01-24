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

#[cfg(feature = "alloc")]
mod serialize {
    use serde_core::Serializer;

    /// Serializes `data` as hex string using lowercase characters.
    ///
    /// Lowercase characters are used (e.g. `f9b4ca`). The resulting string's length
    /// is always even, each byte in data is always encoded using two hex digits.
    /// Thus, the resulting string contains exactly twice as many bytes as the input
    /// data.
    #[inline]
    pub fn serialize<S, T>(data: T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: AsRef<[u8]>,
    {
        serializer.serialize_str(&crate::encode_prefixed(data.as_ref()))
    }

    /// Serializes `data` as hex string using uppercase characters.
    ///
    /// Apart from the characters' casing, this works exactly like [`serialize`].
    #[inline]
    pub fn serialize_upper<S, T>(data: T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: AsRef<[u8]>,
    {
        serializer.serialize_str(&crate::encode_upper_prefixed(data.as_ref()))
    }
}

#[cfg(feature = "alloc")]
pub use serialize::{serialize, serialize_upper};

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

/// Deserializes an optional hex string into raw bytes.
///
/// Returns `None` if the value is null, otherwise deserializes using [`deserialize`].
#[inline]
pub fn deserialize_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromHex,
    <T as FromHex>::Error: fmt::Display,
{
    struct OptionalHexStrVisitor<T>(PhantomData<T>);

    impl<'de, T> Visitor<'de> for OptionalHexStrVisitor<T>
    where
        T: FromHex,
        <T as FromHex>::Error: fmt::Display,
    {
        type Value = Option<T>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a hex encoded string or null")
        }

        fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserialize(deserializer).map(Some)
        }

        fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    deserializer.deserialize_option(OptionalHexStrVisitor(PhantomData))
}

#[cfg(feature = "alloc")]
mod serialize_option {
    use serde_core::Serializer;

    /// Serializes an optional value as a hex string using lowercase characters.
    ///
    /// Serializes `None` as null, and `Some(data)` using [`super::serialize`].
    #[inline]
    pub fn serialize_option<S, T>(data: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: AsRef<[u8]>,
    {
        match data {
            Some(data) => serializer.serialize_str(&crate::encode_prefixed(data.as_ref())),
            None => serializer.serialize_none(),
        }
    }

    /// Serializes an optional value as a hex string using uppercase characters.
    ///
    /// Apart from the characters' casing, this works exactly like [`serialize_option`].
    #[inline]
    pub fn serialize_upper_option<S, T>(data: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: AsRef<[u8]>,
    {
        match data {
            Some(data) => serializer.serialize_str(&crate::encode_upper_prefixed(data.as_ref())),
            None => serializer.serialize_none(),
        }
    }
}

#[cfg(feature = "alloc")]
pub use serialize_option::{serialize_option, serialize_upper_option};
