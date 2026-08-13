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

/// A syntax error in a ClrMamePro datafile, located in the source.
#[cfg(feature = "cmpro")]
#[cfg_attr(docsrs, doc(cfg(feature = "cmpro")))]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}, at line {line} column {column}")]
pub struct CmproError {
    /// What the parser expected at that point.
    pub message: String,
    /// 1-based line number.
    pub line: usize,
    /// 1-based column, counted in characters.
    pub column: usize,
}

/// A `Result` whose error type is [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
