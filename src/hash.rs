//! Strongly-typed checksum values.
//!
//! DAT files store checksums as hex strings. This module parses them into
//! fixed-size values so that they are validated once, compare case-insensitively,
//! and hash cheaply when used as map keys.
//!
//! The digest types are reused from the [RustCrypto] ecosystem rather than
//! redefined, so a checksum read from a DAT is the *same type* as one computed
//! over a file on disk and the two compare directly:
//!
//! ```
//! # use datary::hash::Sha1;
//! # use std::str::FromStr;
//! let from_dat = Sha1::from_str("5177015a91442e56bd76af39447bca365e06c272").unwrap();
//! # #[cfg(feature = "verify")] {
//! let computed = Sha1::compute(b"some rom bytes");
//! assert_ne!(from_dat, computed);
//! # }
//! ```
//!
//! [RustCrypto]: https://github.com/RustCrypto/hashes

use digest::{Output, OutputSizeUser};
use serde::de::{Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

/// An MD5 digest, as found in the `md5` attribute of a `<rom>`.
pub type Md5 = Digest<md5::Md5>;

/// A SHA-1 digest, as found in the `sha1` attribute of a `<rom>`.
pub type Sha1 = Digest<sha1::Sha1>;

/// A SHA-256 digest, as found in the `sha256` attribute of a `<rom>`.
///
/// This attribute is a No-Intro extension and is absent from the Logiqx DTD.
pub type Sha256 = Digest<sha2::Sha256>;

/// The error returned when a checksum string cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HashParseError {
    /// The string did not have the exact number of hex digits the algorithm requires.
    #[error("expected {expected} hex digits, got {actual}")]
    InvalidLength {
        /// Number of hex digits the algorithm requires.
        expected: usize,
        /// Number of characters actually supplied.
        actual: usize,
    },

    /// The string contained a character outside `[0-9a-fA-F]`.
    #[error("invalid hex digit {digit:?} at position {index}")]
    InvalidDigit {
        /// The offending character.
        digit: char,
        /// Byte offset of the offending character.
        index: usize,
    },
}

const fn hex_digit(byte: u8, index: usize) -> Result<u8, HashParseError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(HashParseError::InvalidDigit {
            digit: byte as char,
            index,
        }),
    }
}

/// A CRC32 checksum.
///
/// DAT files write this as up to eight hex digits. Parsing accepts any width up
/// to eight (some older tools omit leading zeroes); [`Display`](fmt::Display)
/// always emits the canonical eight lowercase digits.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Crc32(pub u32);

impl Crc32 {
    /// Returns the checksum as a `u32`.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the checksum as four big-endian bytes.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 4] {
        self.0.to_be_bytes()
    }

    /// Computes the CRC32 of a byte slice.
    #[cfg(feature = "verify")]
    #[cfg_attr(docsrs, doc(cfg(feature = "verify")))]
    #[must_use]
    pub fn compute(bytes: &[u8]) -> Self {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(bytes);
        Self(hasher.finalize())
    }

    /// Computes the CRC32 of everything readable from `reader`.
    #[cfg(feature = "verify")]
    #[cfg_attr(docsrs, doc(cfg(feature = "verify")))]
    pub fn compute_reader(mut reader: impl std::io::Read) -> std::io::Result<Self> {
        let mut hasher = crc32fast::Hasher::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(Self(hasher.finalize()))
    }
}

impl From<u32> for Crc32 {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<Crc32> for u32 {
    fn from(value: Crc32) -> Self {
        value.0
    }
}

impl FromStr for Crc32 {
    type Err = HashParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() || s.len() > 8 {
            return Err(HashParseError::InvalidLength {
                expected: 8,
                actual: s.len(),
            });
        }
        let mut value: u32 = 0;
        for (index, &byte) in s.as_bytes().iter().enumerate() {
            value = (value << 4) | u32::from(hex_digit(byte, index)?);
        }
        Ok(Self(value))
    }
}

impl fmt::Display for Crc32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.0)
    }
}

impl fmt::LowerHex for Crc32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.0)
    }
}

impl fmt::UpperHex for Crc32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08X}", self.0)
    }
}

impl Serialize for Crc32 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Crc32 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Crc32;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a CRC32 checksum as up to 8 hex digits")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Crc32::from_str(v)
                    .map_err(|_| E::invalid_value(Unexpected::Str(v), &"a CRC32 hex string"))
            }
        }
        deserializer.deserialize_str(V)
    }
}

/// A fixed-size cryptographic digest, generic over a [RustCrypto] hash algorithm.
///
/// You will normally use one of the aliases [`Md5`], [`Sha1`] or [`Sha256`]
/// rather than naming this type directly.
///
/// The inner value is [`digest::Output<D>`], so a digest parsed out of a DAT
/// converts freely to and from the output of the corresponding hasher.
///
/// [RustCrypto]: https://github.com/RustCrypto/hashes
pub struct Digest<D: OutputSizeUser>(Output<D>);

impl<D: OutputSizeUser> Digest<D> {
    /// Number of bytes in a digest of this algorithm.
    #[must_use]
    pub fn len() -> usize {
        D::output_size()
    }

    /// Number of hex digits used to write a digest of this algorithm.
    #[must_use]
    pub fn hex_len() -> usize {
        D::output_size() * 2
    }

    /// Returns the raw digest bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Consumes the wrapper and returns the underlying [`digest::Output`].
    #[must_use]
    pub fn into_inner(self) -> Output<D> {
        self.0
    }

    /// Builds a digest from raw bytes, which must be exactly [`Digest::len`] long.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HashParseError> {
        if bytes.len() != D::output_size() {
            return Err(HashParseError::InvalidLength {
                expected: D::output_size(),
                actual: bytes.len(),
            });
        }
        let mut out = Output::<D>::default();
        out.as_mut_slice().copy_from_slice(bytes);
        Ok(Self(out))
    }
}

#[cfg(feature = "verify")]
#[cfg_attr(docsrs, doc(cfg(feature = "verify")))]
impl<D: digest::Digest> Digest<D> {
    /// Computes the digest of a byte slice.
    #[must_use]
    pub fn compute(bytes: &[u8]) -> Self {
        let mut hasher = D::new();
        hasher.update(bytes);
        Self(hasher.finalize())
    }

    /// Computes the digest of everything readable from `reader`.
    pub fn compute_reader(mut reader: impl std::io::Read) -> std::io::Result<Self> {
        let mut hasher = D::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(Self(hasher.finalize()))
    }
}

impl<D: OutputSizeUser> From<Output<D>> for Digest<D> {
    fn from(value: Output<D>) -> Self {
        Self(value)
    }
}

impl<D: OutputSizeUser> AsRef<[u8]> for Digest<D> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl<D: OutputSizeUser> FromStr for Digest<D> {
    type Err = HashParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let expected = D::output_size() * 2;
        if s.len() != expected {
            return Err(HashParseError::InvalidLength {
                expected,
                actual: s.chars().count(),
            });
        }
        let mut out = Output::<D>::default();
        for (i, pair) in s.as_bytes().chunks_exact(2).enumerate() {
            let hi = hex_digit(pair[0], i * 2)?;
            let lo = hex_digit(pair[1], i * 2 + 1)?;
            out[i] = (hi << 4) | lo;
        }
        Ok(Self(out))
    }
}

impl<D: OutputSizeUser> fmt::Display for Digest<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(self, f)
    }
}

impl<D: OutputSizeUser> fmt::LowerHex for Digest<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &byte in self.0.as_slice() {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl<D: OutputSizeUser> fmt::UpperHex for Digest<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &byte in self.0.as_slice() {
            write!(f, "{byte:02X}")?;
        }
        Ok(())
    }
}

impl<D: OutputSizeUser> fmt::Debug for Digest<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:x}")
    }
}

// The derives below are written out by hand: deriving them would add a spurious
// `D: Trait` bound on the algorithm marker type, which never holds.
impl<D: OutputSizeUser> Clone for Digest<D> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// `Array<u8, N>` is `Copy` for every concrete digest size, but that cannot be
// proven for an unresolved `D::OutputSize`, so the bound is spelled out. It
// holds for [`Md5`], [`Sha1`] and [`Sha256`], which is what callers name.
impl<D: OutputSizeUser> Copy for Digest<D> where
    <D::OutputSize as digest::array::ArraySize>::ArrayType<u8>: Copy
{
}

impl<D: OutputSizeUser> Default for Digest<D> {
    fn default() -> Self {
        Self(Output::<D>::default())
    }
}

impl<D: OutputSizeUser> PartialEq for Digest<D> {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice() == other.0.as_slice()
    }
}

impl<D: OutputSizeUser> Eq for Digest<D> {}

impl<D: OutputSizeUser> PartialOrd for Digest<D> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<D: OutputSizeUser> Ord for Digest<D> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.as_slice().cmp(other.0.as_slice())
    }
}

impl<D: OutputSizeUser> Hash for Digest<D> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_slice().hash(state);
    }
}

impl<D: OutputSizeUser> Serialize for Digest<D> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, D: OutputSizeUser> Deserialize<'de> for Digest<D> {
    fn deserialize<De: Deserializer<'de>>(deserializer: De) -> Result<Self, De::Error> {
        struct V<D>(std::marker::PhantomData<D>);
        impl<D: OutputSizeUser> Visitor<'_> for V<D> {
            type Value = Digest<D>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "a digest as {} hex digits", D::output_size() * 2)
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Digest::<D>::from_str(v).map_err(|e| E::custom(e))
            }
        }
        deserializer.deserialize_str(V(std::marker::PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_roundtrip_and_padding() {
        assert_eq!(Crc32::from_str("bb71b522").unwrap(), Crc32(0xbb71_b522));
        assert_eq!(Crc32::from_str("BB71B522").unwrap(), Crc32(0xbb71_b522));
        // Short forms are accepted and normalised on output.
        assert_eq!(Crc32::from_str("456").unwrap().to_string(), "00000456");
        assert_eq!(Crc32(0xbb71_b522).to_string(), "bb71b522");
    }

    #[test]
    fn crc32_rejects_bad_input() {
        assert!(matches!(
            Crc32::from_str("123456789"),
            Err(HashParseError::InvalidLength { .. })
        ));
        assert!(matches!(
            Crc32::from_str(""),
            Err(HashParseError::InvalidLength { .. })
        ));
        assert!(matches!(
            Crc32::from_str("zzzz"),
            Err(HashParseError::InvalidDigit {
                digit: 'z',
                index: 0
            })
        ));
    }

    #[test]
    fn digest_parses_and_is_case_insensitive() {
        let lower = Sha1::from_str("5177015a91442e56bd76af39447bca365e06c272").unwrap();
        let upper = Sha1::from_str("5177015A91442E56BD76AF39447BCA365E06C272").unwrap();
        assert_eq!(lower, upper);
        // Output is always canonical lowercase.
        assert_eq!(
            lower.to_string(),
            "5177015a91442e56bd76af39447bca365e06c272"
        );
        assert_eq!(
            format!("{lower:X}"),
            "5177015A91442E56BD76AF39447BCA365E06C272"
        );
    }

    #[test]
    fn digest_enforces_exact_width() {
        assert_eq!(Sha1::hex_len(), 40);
        assert_eq!(Md5::hex_len(), 32);
        assert_eq!(Sha256::hex_len(), 64);
        // An MD5-width string is not a valid SHA-1.
        assert!(matches!(
            Sha1::from_str("0123456789abcdef0123456789abcdef"),
            Err(HashParseError::InvalidLength {
                expected: 40,
                actual: 32
            })
        ));
    }

    #[test]
    fn digest_from_bytes_checks_length() {
        assert!(Sha1::from_bytes(&[0u8; 20]).is_ok());
        assert!(Sha1::from_bytes(&[0u8; 19]).is_err());
    }

    #[cfg(feature = "verify")]
    #[test]
    fn computed_digest_equals_parsed_digest() {
        // The whole point of reusing RustCrypto types: these are the same type.
        let computed = Sha1::compute(b"hello");
        let parsed = Sha1::from_str("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d").unwrap();
        assert_eq!(computed, parsed);

        assert_eq!(
            Md5::compute(b"hello").to_string(),
            "5d41402abc4b2a76b9719d911017c592"
        );
        assert_eq!(Crc32::compute(b"hello").to_string(), "3610a686");
    }

    #[cfg(feature = "verify")]
    #[test]
    fn reader_and_slice_hashing_agree() {
        let data = vec![7u8; 200_000];
        assert_eq!(
            Sha256::compute(&data),
            Sha256::compute_reader(&data[..]).unwrap()
        );
        assert_eq!(
            Crc32::compute(&data),
            Crc32::compute_reader(&data[..]).unwrap()
        );
    }
}
