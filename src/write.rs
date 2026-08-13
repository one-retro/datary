//! Control over how a datafile is serialised.
//!
//! The defaults produce conventional XML: an declaration, tab indentation, LF
//! line endings and a trailing newline. [`WriteOptions::no_intro`] instead
//! reproduces No-Intro's exact house style, which makes output byte-identical
//! to the datafiles published by DAT-o-MATIC:
//!
//! ```no_run
//! use datary::write::WriteOptions;
//!
//! let dat = datary::read_file("Nintendo - Virtual Boy.dat")?;
//! let xml = datary::to_string_with(&dat, &WriteOptions::no_intro())?;
//! # Ok::<(), datary::Error>(())
//! ```

use crate::dat::Datafile;
use crate::error::Result;
use crate::format::DatFormat;
use serde::Serialize;
use std::io::Write;
use std::path::Path;

/// The line terminator to write.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineEnding {
    /// `\n`. The default.
    #[default]
    Lf,
    /// `\r\n`. Used by published No-Intro datafiles.
    Crlf,
}

impl LineEnding {
    /// The terminator as a string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

/// How to serialise a [`Datafile`].
///
/// These are formatting concerns only; *which syntax* to write is chosen by the
/// [`DatFormat`] doing the writing. Options that make
/// no sense for a given syntax are ignored by it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOptions {
    /// Line terminator. Defaults to [`LineEnding::Lf`].
    pub line_ending: LineEnding,

    /// Indent character and the number of them per level, or [`None`] to write
    /// everything on one line. Defaults to one tab.
    pub indent: Option<(char, usize)>,

    /// Whether to write an `<?xml version="1.0"?>` declaration. Defaults to
    /// `true`. Ignored by syntaxes that have no such construct.
    pub declaration: bool,

    /// Whether to end the document with a line terminator. Defaults to `true`;
    /// No-Intro datafiles do not have one.
    pub trailing_newline: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            line_ending: LineEnding::Lf,
            indent: Some(('\t', 1)),
            declaration: true,
            trailing_newline: true,
        }
    }
}

impl WriteOptions {
    /// Options matching the formatting of published No-Intro datafiles: tab
    /// indentation, CRLF line endings, an XML declaration and no trailing
    /// newline.
    #[must_use]
    pub fn no_intro() -> Self {
        Self {
            line_ending: LineEnding::Crlf,
            indent: Some(('\t', 1)),
            declaration: true,
            trailing_newline: false,
        }
    }

    /// Options for the native ClrMamePro syntax: tab indentation, LF line
    /// endings and a trailing newline.
    #[cfg(feature = "cmpro")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cmpro")))]
    #[must_use]
    pub fn clrmamepro() -> Self {
        Self {
            line_ending: LineEnding::Lf,
            indent: Some(('\t', 1)),
            declaration: false,
            trailing_newline: true,
        }
    }

    /// Options producing the smallest output: no indentation, no declaration
    /// and no trailing newline.
    #[must_use]
    pub fn compact() -> Self {
        Self {
            line_ending: LineEnding::Lf,
            indent: None,
            declaration: false,
            trailing_newline: false,
        }
    }

    /// Sets the line ending.
    #[must_use]
    pub const fn line_ending(mut self, line_ending: LineEnding) -> Self {
        self.line_ending = line_ending;
        self
    }

    /// Sets the indentation.
    #[must_use]
    pub const fn indent(mut self, character: char, count: usize) -> Self {
        self.indent = Some((character, count));
        self
    }
}

/// Serialises a datafile as XML using the given options.
///
/// Use [`crate::to_string_as`] for another syntax.
///
/// # Errors
///
/// Returns [`crate::Error::Serialize`] if the datafile cannot be serialised.
pub fn to_string_with(dat: &Datafile, options: &WriteOptions) -> Result<String> {
    to_xml_string(dat, options)
}

/// The XML writer proper, called by [`crate::format::Xml`].
pub(crate) fn to_xml_string(dat: &Datafile, options: &WriteOptions) -> Result<String> {
    let mut out = String::new();

    if options.declaration {
        out.push_str("<?xml version=\"1.0\"?>\n");
    }

    let mut serializer = quick_xml::se::Serializer::with_root(&mut out, Some("datafile"))?;
    if let Some((character, count)) = options.indent {
        serializer.indent(character, count);
    }
    dat.serialize(serializer)?;

    if options.trailing_newline {
        out.push('\n');
    }

    if options.line_ending == LineEnding::Crlf {
        // quick-xml only ever emits bare LF. Rewriting every LF is safe for
        // content too: XML parsers normalise CRLF back to LF on the way in.
        out = out.replace('\n', "\r\n");
    }

    Ok(out)
}

/// Serialises a datafile as XML to any [`Write`], using the given options.
///
/// Use [`to_writer_as`] for another syntax.
///
/// # Errors
///
/// Returns [`crate::Error::Serialize`] if the datafile cannot be serialised, or
/// [`crate::Error::Io`] if the writer fails.
pub fn to_writer_with(writer: impl Write, dat: &Datafile, options: &WriteOptions) -> Result<()> {
    to_writer_as(writer, dat, &crate::format::Xml, options)
}

/// Serialises a datafile in the given syntax to any [`Write`].
///
/// # Errors
///
/// Returns whatever the format reports, or [`crate::Error::Io`] if the writer
/// fails.
pub fn to_writer_as(
    mut writer: impl Write,
    dat: &Datafile,
    format: &dyn DatFormat,
    options: &WriteOptions,
) -> Result<()> {
    writer.write_all(format.write(dat, options)?.as_bytes())?;
    Ok(())
}

/// Writes a datafile as XML to a path, replacing any existing file.
///
/// Use [`write_file_as`] for another syntax.
///
/// # Errors
///
/// Returns [`crate::Error::Serialize`] if the datafile cannot be serialised, or
/// [`crate::Error::Io`] if the file cannot be written.
pub fn write_file_with(
    path: impl AsRef<Path>,
    dat: &Datafile,
    options: &WriteOptions,
) -> Result<()> {
    write_file_as(path, dat, &crate::format::Xml, options)
}

/// Writes a datafile in the given syntax to a path, replacing any existing file.
///
/// # Errors
///
/// Returns whatever the format reports, or [`crate::Error::Io`] if the file
/// cannot be written.
pub fn write_file_as(
    path: impl AsRef<Path>,
    dat: &Datafile,
    format: &dyn DatFormat,
    options: &WriteOptions,
) -> Result<()> {
    let file = std::fs::File::create(path.as_ref())?;
    to_writer_as(std::io::BufWriter::new(file), dat, format, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Datafile {
        crate::from_str(
            r#"<datafile><game name="g"><description>d</description>
                 <rom name="g.rom" size="1" crc="00000001"/>
               </game></datafile>"#,
        )
        .unwrap()
    }

    #[test]
    fn default_options_use_lf_and_tabs() {
        let xml = to_string_with(&sample(), &WriteOptions::default()).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\"?>\n"));
        assert!(!xml.contains('\r'));
        assert!(xml.contains("\n\t<game"));
        assert!(xml.ends_with("</datafile>\n"));
    }

    #[test]
    fn no_intro_options_use_crlf_and_no_trailing_newline() {
        let xml = to_string_with(&sample(), &WriteOptions::no_intro()).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\"?>\r\n"));
        assert!(xml.ends_with("</datafile>"));
        assert!(!xml.ends_with('\n'));
        // Every LF must be part of a CRLF pair.
        assert_eq!(xml.matches('\n').count(), xml.matches("\r\n").count());
    }

    #[test]
    fn compact_options_emit_one_line() {
        let xml = to_string_with(&sample(), &WriteOptions::compact()).unwrap();
        assert!(!xml.contains('\n'));
        assert!(!xml.starts_with("<?xml"));
    }

    #[test]
    fn builders_override_defaults() {
        let opts = WriteOptions::default()
            .line_ending(LineEnding::Crlf)
            .indent(' ', 4);
        let xml = to_string_with(&sample(), &opts).unwrap();
        assert!(xml.contains("\r\n    <game"));
        assert_eq!(LineEnding::Crlf.as_str(), "\r\n");
    }
}
