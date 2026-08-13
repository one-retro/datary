//! Datafile syntaxes, and the trait for adding more.
//!
//! A *format* is a syntax for the same [`Datafile`] model. This crate ships two
//! — [`Xml`] and [`ClrMamePro`] — but the [`DatFormat`] trait is public, so a
//! downstream crate can add another (RomCenter's INI dialects, `attract-mode`,
//! a bespoke in-house format) and have it work with the same reading, writing
//! and detection helpers.
//!
//! ```
//! use datary::format::{ClrMamePro, DatFormat, Xml};
//!
//! # #[cfg(feature = "cmpro")] {
//! let dat = Xml.parse("<datafile><game name=\"g\"><description>g</description></game></datafile>")?;
//!
//! // The same model round-trips through either syntax.
//! let text = ClrMamePro.write(&dat, &datary::WriteOptions::clrmamepro())?;
//! assert_eq!(ClrMamePro.parse(&text)?, dat);
//! # }
//! # Ok::<(), datary::Error>(())
//! ```
//!
//! # Implementing a format
//!
//! ```
//! use datary::format::DatFormat;
//! use datary::{Datafile, Result, WriteOptions};
//!
//! struct Sfv;
//!
//! impl DatFormat for Sfv {
//!     fn name(&self) -> &'static str { "SFV" }
//!
//!     fn detect(&self, bytes: &[u8]) -> bool {
//!         bytes.starts_with(b"; ")
//!     }
//!
//!     fn parse(&self, _source: &str) -> Result<Datafile> {
//!         Ok(Datafile::default())
//!     }
//!
//!     fn write(&self, _dat: &Datafile, _options: &WriteOptions) -> Result<String> {
//!         Ok(String::new())
//!     }
//! }
//!
//! // Usable anywhere a built-in format is.
//! let dat = datary::from_str_as("; a comment", &Sfv)?;
//! # Ok::<(), datary::Error>(())
//! ```

use crate::dat::Datafile;
use crate::error::Result;
use crate::write::WriteOptions;

/// A datafile syntax: how to recognise it, read it and write it.
///
/// Implementations are normally unit structs, since a format carries no state.
/// The trait is object-safe, so `&dyn DatFormat` works throughout.
pub trait DatFormat {
    /// A short human-readable name, used in diagnostics.
    fn name(&self) -> &'static str;

    /// Whether this format's parser is likely to accept `bytes`.
    ///
    /// Implementations should look only at a short prefix and must not be
    /// expensive: detection calls this on every candidate format in turn.
    /// Returning `false` for ambiguous input is better than claiming it, since
    /// the first format to answer `true` wins.
    fn detect(&self, bytes: &[u8]) -> bool;

    /// Parses a datafile.
    ///
    /// # Errors
    ///
    /// Returns whichever [`Error`](crate::Error) variant describes the failure.
    fn parse(&self, source: &str) -> Result<Datafile>;

    /// Serialises a datafile.
    ///
    /// Implementations should honour whichever [`WriteOptions`] fields make
    /// sense for the syntax and ignore the rest.
    ///
    /// # Errors
    ///
    /// Returns whichever [`Error`](crate::Error) variant describes the failure.
    fn write(&self, dat: &Datafile, options: &WriteOptions) -> Result<String>;
}

/// The Logiqx/No-Intro XML syntax.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Xml;

impl DatFormat for Xml {
    fn name(&self) -> &'static str {
        "XML"
    }

    /// XML is the only syntax here that starts with `<`, ignoring a
    /// byte-order mark and leading whitespace.
    fn detect(&self, bytes: &[u8]) -> bool {
        first_meaningful_byte(bytes) == Some(b'<')
    }

    fn parse(&self, source: &str) -> Result<Datafile> {
        Ok(quick_xml::de::from_str(source)?)
    }

    fn write(&self, dat: &Datafile, options: &WriteOptions) -> Result<String> {
        crate::write::to_xml_string(dat, options)
    }
}

/// The native ClrMamePro syntax. See [`crate::cmpro`].
#[cfg(feature = "cmpro")]
#[cfg_attr(docsrs, doc(cfg(feature = "cmpro")))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClrMamePro;

#[cfg(feature = "cmpro")]
impl DatFormat for ClrMamePro {
    fn name(&self) -> &'static str {
        "ClrMamePro"
    }

    /// Claims anything non-empty that is not XML. This is deliberately the
    /// permissive fallback, so it must be tried last during detection.
    fn detect(&self, bytes: &[u8]) -> bool {
        first_meaningful_byte(bytes).is_some_and(|b| b != b'<')
    }

    fn parse(&self, source: &str) -> Result<Datafile> {
        crate::cmpro::from_str(source)
    }

    fn write(&self, dat: &Datafile, options: &WriteOptions) -> Result<String> {
        Ok(crate::cmpro::to_string_with(dat, options))
    }
}

/// First byte that is neither a UTF-8 byte-order mark nor whitespace.
fn first_meaningful_byte(bytes: &[u8]) -> Option<u8> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    bytes.iter().copied().find(|b| !b.is_ascii_whitespace())
}

/// The formats this build knows about, in detection order.
///
/// Ordered most specific first, because [`detect`] takes the first match and
/// [`ClrMamePro`] deliberately claims anything that is not XML.
#[cfg(feature = "cmpro")]
pub const BUILTIN_FORMATS: &[&dyn DatFormat] = &[&Xml, &ClrMamePro];

/// The formats this build knows about, in detection order.
#[cfg(not(feature = "cmpro"))]
pub const BUILTIN_FORMATS: &[&dyn DatFormat] = &[&Xml];

/// Identifies which of `formats` should read `bytes`, taking the first match.
///
/// Pass [`BUILTIN_FORMATS`] for the built-in set, or your own slice to include
/// a custom format:
///
/// ```
/// use datary::format::{detect, Xml, BUILTIN_FORMATS};
///
/// let format = detect(b"<datafile/>", BUILTIN_FORMATS).unwrap();
/// assert_eq!(format.name(), "XML");
/// assert!(detect(b"   ", BUILTIN_FORMATS).is_none());
/// ```
#[must_use]
pub fn detect<'a>(bytes: &[u8], formats: &'a [&'a dyn DatFormat]) -> Option<&'a dyn DatFormat> {
    formats.iter().copied().find(|f| f.detect(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_is_detected() {
        assert!(Xml.detect(b"<datafile/>"));
        assert!(Xml.detect(b"  \r\n\t<datafile/>"));
        assert!(Xml.detect(b"\xEF\xBB\xBF<?xml version=\"1.0\"?>"));
        assert!(!Xml.detect(b"clrmamepro ("));
        assert!(!Xml.detect(b""));
    }

    #[cfg(feature = "cmpro")]
    #[test]
    fn clrmamepro_claims_everything_that_is_not_xml() {
        assert!(ClrMamePro.detect(b"clrmamepro (\n"));
        assert!(ClrMamePro.detect(b"\n\nBEGIN\n"));
        assert!(ClrMamePro.detect(b"emulator (\n"));
        assert!(!ClrMamePro.detect(b"<datafile/>"));
        assert!(!ClrMamePro.detect(b"   \n\t"));
    }

    #[test]
    fn detection_finds_nothing_in_empty_input() {
        assert!(detect(b"", BUILTIN_FORMATS).is_none());
        assert!(detect(b"   \n\t\r ", BUILTIN_FORMATS).is_none());
        assert!(detect(b"\xEF\xBB\xBF", BUILTIN_FORMATS).is_none());
    }

    #[test]
    fn detection_picks_xml_over_the_fallback() {
        let f = detect(b"<datafile/>", BUILTIN_FORMATS).unwrap();
        assert_eq!(f.name(), "XML");
    }

    #[cfg(feature = "cmpro")]
    #[test]
    fn detection_falls_back_to_clrmamepro() {
        let f = detect(b"game ( name x )", BUILTIN_FORMATS).unwrap();
        assert_eq!(f.name(), "ClrMamePro");
    }

    /// A downstream format must compose with the built-in helpers.
    #[test]
    fn a_custom_format_can_be_detected_and_parsed() {
        struct Empty;
        impl DatFormat for Empty {
            fn name(&self) -> &'static str {
                "Empty"
            }
            fn detect(&self, bytes: &[u8]) -> bool {
                bytes.starts_with(b"#empty")
            }
            fn parse(&self, _: &str) -> Result<Datafile> {
                Ok(Datafile::default())
            }
            fn write(&self, _: &Datafile, _: &WriteOptions) -> Result<String> {
                Ok(String::new())
            }
        }

        let formats: &[&dyn DatFormat] = &[&Empty, &Xml];
        let f = detect(b"#empty", formats).unwrap();
        assert_eq!(f.name(), "Empty");
        assert_eq!(f.parse("#empty").unwrap(), Datafile::default());

        // ...and does not shadow the built-ins.
        assert_eq!(detect(b"<datafile/>", formats).unwrap().name(), "XML");
    }
}
