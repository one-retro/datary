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
//! # Feature flags
//!
//! * `index` (default) — [`index::IndexedDatafile`], lookup tables by checksum,
//!   size and name.
//! * `verify` (default) — hashing files and checking them against a datafile.
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
pub mod hash;
pub mod write;

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
pub use hash::{Crc32, HashParseError, Md5, Sha1, Sha256};
pub use write::{to_string_with, to_writer_with, write_file_with, LineEnding, WriteOptions};

#[cfg(feature = "index")]
pub use index::{Index, IndexedDatafile, RomRef};

use std::io::{BufReader, Read, Write};
use std::path::Path;

/// Reads a datafile from a path.
///
/// # Errors
///
/// Returns [`Error::Io`] if the file cannot be opened or read, and
/// [`Error::Xml`] if its contents are not a valid datafile.
pub fn read_file(path: impl AsRef<Path>) -> Result<Datafile> {
    let file = std::fs::File::open(path.as_ref())?;
    from_reader(file)
}

/// Reads a datafile from any [`Read`].
///
/// The reader is buffered internally, so there is no need to wrap it.
///
/// # Errors
///
/// Returns [`Error::Xml`] if the contents are not a valid datafile.
pub fn from_reader(reader: impl Read) -> Result<Datafile> {
    let mut reader = BufReader::new(reader);
    Ok(quick_xml::de::from_reader(&mut reader)?)
}

/// Reads a datafile from a string.
///
/// # Errors
///
/// Returns [`Error::Xml`] if the contents are not a valid datafile.
pub fn from_str(xml: &str) -> Result<Datafile> {
    Ok(quick_xml::de::from_str(xml)?)
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
