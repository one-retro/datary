//! Read and write DAT ROM datafiles, fully modelled in Rust's type system.
//!
//! `datary` parses the XML datafiles published by [No-Intro], [Redump], [TOSEC]
//! and other preservation projects. Every enumerated attribute becomes a Rust
//! enum and every checksum becomes a fixed-size value, so a datafile is
//! validated once at parse time instead of being passed around as a bag of
//! strings.
//!
//! # Reading
//!
//! ```no_run
//! let dat = datary::read_file("Nintendo - Virtual Boy.dat")?;
//!
//! let header = dat.header.as_ref().unwrap();
//! println!("{} ({})", header.name, header.version);
//!
//! for game in &dat.games {
//!     for rom in &game.roms {
//!         println!("{} {} {:?}", rom.name, rom.size, rom.sha1);
//!     }
//! }
//! # Ok::<(), datary::Error>(())
//! ```
//!
//! # Checksums are typed
//!
//! Checksums are reused from the [RustCrypto] hash crates rather than kept as
//! strings, so a value read from a datafile is the same type as one computed
//! over a file on disk and the two compare directly — no case normalisation, no
//! length guessing:
//!
//! ```
//! use datary::hash::Sha1;
//!
//! # #[cfg(feature = "verify")] {
//! let dat = datary::from_str(r#"
//!     <datafile><game name="g"><description>g</description>
//!         <rom name="g.rom" size="5" sha1="aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"/>
//!     </game></datafile>"#)?;
//!
//! let rom = &dat.games[0].roms[0];
//! assert_eq!(rom.sha1, Some(Sha1::compute(b"hello")));
//! # }
//! # Ok::<(), datary::Error>(())
//! ```
//!
//! # Enumerations are typed
//!
//! ```
//! use datary::{Status, YesNo};
//!
//! let dat = datary::from_str(r#"
//!     <datafile><game name="g"><description>g</description>
//!         <rom name="g.rom" size="0" status="nodump" mia="yes"/>
//!     </game></datafile>"#)?;
//!
//! let rom = &dat.games[0].roms[0];
//! assert_eq!(rom.status(), Status::NoDump);
//! assert_eq!(rom.mia, Some(YesNo::Yes));
//! assert!(rom.is_mia());
//! # Ok::<(), datary::Error>(())
//! ```
//!
//! # Which dialect?
//!
//! One set of types covers all of them. See [`dat`] for exactly which fields
//! come from which specification.
//!
//! # Checking a datafile
//!
//! Parsing is permissive, so a dangling `cloneof` is not an error. Referential
//! integrity is reported separately by [`Datafile::validate`], which never
//! fails a file:
//!
//! ```
//! let dat = datary::from_str(r#"
//!     <datafile>
//!         <game name="Child" cloneof="Missing"><description>Child</description></game>
//!     </datafile>"#)?;
//!
//! for issue in dat.validate() {
//!     println!("{issue}");
//! }
//! # Ok::<(), datary::Error>(())
//! ```
//!
//! # Which syntax?
//!
//! Both the Logiqx XML syntax and ClrMamePro's native one parse into the same
//! [`Datafile`], and [`read_file`] detects which it was given. See [`mod@format`]
//! to choose explicitly, or to add a syntax of your own.
//!
//! # Feature flags
//!
//! * `index` (default) — [`index::IndexedDatafile`], lookup tables by checksum,
//!   size and name.
//! * `verify` (default) — hashing files and checking them against a datafile.
//! * `cmpro` (default) — the native ClrMamePro syntax, via [`cmpro`] and
//!   [`format::ClrMamePro`].
//!
//! [No-Intro]: https://no-intro.org
//! [Redump]: http://redump.org
//! [TOSEC]: https://www.tosecdev.org
//! [RustCrypto]: https://github.com/RustCrypto/hashes

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::missing_errors_doc)]

pub mod dat;
pub mod enums;
pub mod error;
pub mod format;
pub mod hash;
pub mod validate;
pub mod write;

#[cfg(feature = "cmpro")]
#[cfg_attr(docsrs, doc(cfg(feature = "cmpro")))]
pub mod cmpro;

#[cfg(feature = "index")]
#[cfg_attr(docsrs, doc(cfg(feature = "index")))]
pub mod index;

#[cfg(feature = "verify")]
#[cfg_attr(docsrs, doc(cfg(feature = "verify")))]
pub mod verify;

pub use dat::{
    Archive, BiosSet, ClrMamePro, Datafile, Disk, Game, Header, Release, Rom, RomCenter, Sample,
};
pub use enums::{ForceMerging, ForceNoDump, ForcePacking, RomMode, SampleMode, Status, YesNo};
pub use error::{Error, Result};
// The format markers live in `format` rather than at the root: `ClrMamePro`
// here would collide with `dat::ClrMamePro`, the `<clrmamepro>` header element.
pub use format::{detect, DatFormat, BUILTIN_FORMATS};
pub use hash::{Crc32, HashParseError, Md5, Sha1, Sha256};
pub use validate::{Issue, IssueKind};

#[cfg(feature = "cmpro")]
pub use error::{CmproError, Position};
pub use write::{
    to_string_with, to_writer_as, to_writer_with, write_file_as, write_file_with, LineEnding,
    WriteOptions,
};

#[cfg(feature = "index")]
pub use index::{Index, IndexedDatafile, RomRef};

use std::io::{BufReader, Read, Write};
use std::path::Path;

/// Decodes bytes as UTF-8, reporting an encoding failure as such.
pub(crate) fn decode_utf8(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes).map_err(|e| Error::Encoding {
        valid_up_to: e.valid_up_to(),
    })
}

/// Decodes ISO-8859-1 (Latin-1) bytes.
///
/// Every byte is valid in that encoding — its 256 values map onto the first 256
/// code points — so this cannot fail. That is exactly why it must not be used
/// as a blind fallback: it will happily turn genuinely broken UTF-8 into
/// plausible-looking mojibake. Reach for it when you know the file is Latin-1,
/// which for datafiles usually means older TOSEC or ClrMamePro output.
///
/// ```
/// // 0xE9 is `é` in ISO-8859-1, and invalid on its own in UTF-8.
/// assert_eq!(datary::decode_latin1(b"Pok\xe9mon"), "Pokémon");
/// ```
#[must_use]
pub fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// Reads a datafile from a path, detecting its syntax from the contents.
///
/// Both syntaxes conventionally use a `.dat` extension, so the file's leading
/// bytes decide: `<` means XML, anything else means ClrMamePro. Use
/// [`read_file_as`] to force one.
///
/// # Errors
///
/// Returns [`Error::Io`] if the file cannot be opened or read,
/// [`Error::UnknownFormat`] if it is empty, and [`Error::Xml`] or
/// [`Error::Cmpro`] if its contents are malformed.
pub fn read_file(path: impl AsRef<Path>) -> Result<Datafile> {
    from_bytes(&std::fs::read(path.as_ref())?)
}

/// Reads a datafile from bytes, detecting its syntax from the contents.
///
/// # Errors
///
/// Returns [`Error::Encoding`] if the bytes are not UTF-8, plus the errors
/// [`from_str`] can return.
pub fn from_bytes(bytes: &[u8]) -> Result<Datafile> {
    from_str(decode_utf8(bytes)?)
}

/// Reads a datafile from a path, in a known syntax.
///
/// # Errors
///
/// As [`read_file`], but never returns [`Error::UnknownFormat`].
pub fn read_file_as(path: impl AsRef<Path>, format: &dyn DatFormat) -> Result<Datafile> {
    format.parse(decode_utf8(&std::fs::read(path.as_ref())?)?)
}

/// Reads a datafile from any [`Read`], detecting its syntax from the contents.
///
/// # Errors
///
/// As [`read_file`].
pub fn from_reader(reader: impl Read) -> Result<Datafile> {
    let mut bytes = Vec::new();
    BufReader::new(reader).read_to_end(&mut bytes)?;
    from_bytes(&bytes)
}

/// Reads a datafile from any [`Read`], in a known syntax.
///
/// # Errors
///
/// As [`read_file_as`].
pub fn from_reader_as(reader: impl Read, format: &dyn DatFormat) -> Result<Datafile> {
    let mut bytes = Vec::new();
    BufReader::new(reader).read_to_end(&mut bytes)?;
    format.parse(decode_utf8(&bytes)?)
}

/// Reads a datafile from a string, detecting its syntax from the contents.
///
/// # Errors
///
/// Returns [`Error::UnknownFormat`] if the input is empty or entirely
/// whitespace, and [`Error::Xml`] or [`Error::Cmpro`] if it is malformed.
pub fn from_str(source: &str) -> Result<Datafile> {
    let format = detect(source.as_bytes(), BUILTIN_FORMATS).ok_or(Error::UnknownFormat)?;
    format.parse(source)
}

/// Reads a datafile from a string, in a known syntax.
///
/// # Errors
///
/// Returns [`Error::Xml`] or [`Error::Cmpro`] if the input is malformed.
pub fn from_str_as(source: &str, format: &dyn DatFormat) -> Result<Datafile> {
    format.parse(source)
}

/// Serialises a datafile to a string, indented with tabs.
///
/// Use [`to_string_with`] to control the formatting — in particular
/// [`WriteOptions::no_intro`], which reproduces No-Intro's output byte for byte.
///
/// # Errors
///
/// Returns [`Error::Serialize`] if the datafile cannot be serialised.
pub fn to_string(dat: &Datafile) -> Result<String> {
    to_string_with(dat, &WriteOptions::default())
}

/// Serialises a datafile in the given syntax.
///
/// # Errors
///
/// Returns whatever the format reports.
pub fn to_string_as(
    dat: &Datafile,
    format: &dyn DatFormat,
    options: &WriteOptions,
) -> Result<String> {
    format.write(dat, options)
}

/// Serialises a datafile to any [`Write`], as UTF-8.
///
/// # Errors
///
/// Returns [`Error::Serialize`] if the datafile cannot be serialised, or
/// [`Error::Io`] if the writer fails.
pub fn to_writer(writer: impl Write, dat: &Datafile) -> Result<()> {
    to_writer_with(writer, dat, &WriteOptions::default())
}

/// Writes a datafile to a path, replacing any existing file.
///
/// # Errors
///
/// Returns [`Error::Serialize`] if the datafile cannot be serialised, or
/// [`Error::Io`] if the file cannot be written.
pub fn write_file(path: impl AsRef<Path>, dat: &Datafile) -> Result<()> {
    write_file_with(path, dat, &WriteOptions::default())
}
