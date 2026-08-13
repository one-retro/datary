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

    /// Where in the source, when the error is tied to one.
    ///
    /// Syntax errors carry a position. Errors found while interpreting the
    /// parsed blocks — an invalid checksum value, say — do not, because the
    /// block tree does not retain spans.
    pub position: Option<Position>,
}

/// A 1-based position in a source file.
#[cfg(feature = "cmpro")]
#[cfg_attr(docsrs, doc(cfg(feature = "cmpro")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column, counted in characters rather than bytes.
    pub column: usize,
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
