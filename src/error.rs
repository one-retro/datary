//! Error types.

use crate::hash::HashParseError;

/// Any error produced while reading or writing a datafile.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The underlying reader or writer failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The document was not well-formed XML, or did not match the model.
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::DeError),

    /// The datafile could not be serialised to XML.
    #[error("XML serialization error: {0}")]
    Serialize(#[from] quick_xml::SeError),

    /// A checksum attribute was not valid hex of the expected width.
    #[error("invalid checksum: {0}")]
    Hash(#[from] HashParseError),

    /// A ClrMamePro datafile was malformed.
    #[cfg(feature = "cmpro")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cmpro")))]
    #[error("{0}")]
    Cmpro(#[from] CmproError),

    /// The input was empty, so its syntax could not be determined.
    ///
    /// There is no "unsupported format" case to go with this: formats are
    /// types, so one whose support was compiled out cannot be named at all.
    #[error("cannot determine datafile format: input is empty")]
    UnknownFormat,
}

/// An error in a ClrMamePro datafile.
#[cfg(feature = "cmpro")]
#[cfg_attr(docsrs, doc(cfg(feature = "cmpro")))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmproError {
    /// What went wrong.
    pub message: String,

    /// Where in the source the error occurred.
    ///
    /// This is `None` only for an error not derived from source text at all;
    /// both syntax errors and bad values carry a position.
    pub position: Option<Position>,
}

/// A location in a source file.
///
/// Carries the raw byte `offset` as well as the human-facing line and column,
/// so the error can drive a diagnostic renderer (`ariadne`, `miette`,
/// `codespan-reporting`) that wants a byte span, without the caller having to
/// recompute it.
#[cfg(feature = "cmpro")]
#[cfg_attr(docsrs, doc(cfg(feature = "cmpro")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// 0-based byte offset from the start of the source.
    pub offset: usize,
    /// 1-based line number.
    pub line: usize,
    /// 1-based column, counted in characters rather than bytes.
    pub column: usize,
}

#[cfg(feature = "cmpro")]
impl Position {
    /// Locates a byte offset within `source`.
    ///
    /// The offset is clamped to the length of the source, so an
    /// end-of-input error resolves to the final position rather than panicking.
    #[must_use]
    pub fn from_offset(source: &str, offset: usize) -> Self {
        let offset = offset.min(source.len());
        // Round down to a char boundary; an offset can land mid-character only
        // if a caller passes one in by hand.
        let offset = (0..=offset)
            .rev()
            .find(|i| source.is_char_boundary(*i))
            .unwrap_or(0);

        let before = &source[..offset];
        let line = before.matches('\n').count() + 1;
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        // Count characters rather than bytes so non-ASCII names report sensibly.
        let column = source[line_start..offset].chars().count() + 1;

        Self {
            offset,
            line,
            column,
        }
    }

    /// Locates `substring` within `source`, which must be a slice borrowed
    /// from it.
    ///
    /// Values in the block tree are borrowed directly out of the source, so
    /// their position is recoverable from the slice itself — no span needs to
    /// be threaded through the parser. Returns [`None`] if `substring` is not
    /// part of `source`.
    #[must_use]
    pub fn of_substring(source: &str, substring: &str) -> Option<Self> {
        let base = source.as_ptr() as usize;
        let start = substring.as_ptr() as usize;
        let end = start.checked_add(substring.len())?;

        (start >= base && end <= base + source.len())
            .then(|| Self::from_offset(source, start - base))
    }
}

#[cfg(feature = "cmpro")]
impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {} column {}", self.line, self.column)
    }
}

#[cfg(feature = "cmpro")]
impl std::fmt::Display for CmproError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.position {
            Some(position) => write!(f, "{}, at {position}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

#[cfg(feature = "cmpro")]
impl std::error::Error for CmproError {}

/// A `Result` whose error type is [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(all(test, feature = "cmpro"))]
mod tests {
    use super::*;

    #[test]
    fn from_offset_reports_line_column_and_offset() {
        let source = "one\ntwo\nthree";
        let p = Position::from_offset(source, 4);
        assert_eq!((p.offset, p.line, p.column), (4, 2, 1));

        let p = Position::from_offset(source, 9);
        assert_eq!((p.offset, p.line, p.column), (9, 3, 2));
    }

    #[test]
    fn columns_count_characters_but_offsets_count_bytes() {
        // 'é' is two bytes, so byte offset 3 is the third character.
        let p = Position::from_offset("aéb\nx", 3);
        assert_eq!(p.offset, 3, "offset stays in bytes");
        assert_eq!((p.line, p.column), (1, 3), "column counts characters");

        let p = Position::from_offset("aéb\nx", 5);
        assert_eq!((p.line, p.column), (2, 1));
    }

    #[test]
    fn out_of_range_offsets_are_clamped() {
        let source = "abc";
        assert_eq!(Position::from_offset(source, 99).offset, 3);
        // Landing mid-character rounds down to a boundary rather than panicking.
        assert_eq!(Position::from_offset("é", 1).offset, 0);
    }

    #[test]
    fn a_borrowed_slice_locates_itself() {
        let source = "clrmamepro (\n\tname Test\n)";
        let needle = &source[19..23]; // "Test"
        assert_eq!(needle, "Test");

        let p = Position::of_substring(source, needle).unwrap();
        assert_eq!(p.offset, 19);
        assert_eq!((p.line, p.column), (2, 7));
    }

    #[test]
    fn a_foreign_slice_does_not_locate() {
        let source = String::from("abcdef");
        let other = String::from("abcdef");
        assert!(Position::of_substring(&source, &other).is_none());
        // Empty tail slice at the very end is still inside.
        assert!(Position::of_substring(&source, &source[6..]).is_some());
    }

    #[test]
    fn display_omits_a_missing_position() {
        let located = CmproError {
            message: "bad".into(),
            position: Some(Position {
                offset: 5,
                line: 2,
                column: 3,
            }),
        };
        assert_eq!(located.to_string(), "bad, at line 2 column 3");

        let unlocated = CmproError {
            message: "bad".into(),
            position: None,
        };
        assert_eq!(unlocated.to_string(), "bad");
    }
}
