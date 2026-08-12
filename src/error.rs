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
}

/// A `Result` whose error type is [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
