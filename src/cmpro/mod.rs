//! The native ClrMamePro datafile syntax.
//!
//! DAT-o-MATIC and most preservation projects publish datafiles in two
//! syntaxes: the Logiqx XML this crate reads by default, and ClrMamePro's own
//! brace-delimited text format:
//!
//! ```text
//! clrmamepro (
//!     name "Nintendo - Virtual Boy"
//!     version 20240829-133848
//! )
//!
//! game (
//!     name "3-D Tetris (USA)"
//!     description "3-D Tetris (USA)"
//!     rom ( name "3-D Tetris (USA).vb" size 1048576 crc bb71b522 )
//! )
//! ```
//!
//! Both parse into the same [`Datafile`], so everything
//! downstream — [`index`](crate::index), [`verify`](crate::verify), the helper
//! methods — works identically whichever syntax a file arrived in, and a file
//! read in one can be written in the other.
//!
//! ```
//! # #[cfg(feature = "cmpro")] {
//! let dat = datary::cmpro::from_str(r#"
//!     game (
//!         name "Some Game"
//!         description "Some Game"
//!         rom ( name some.rom size 8 crc 3610a686 )
//!     )"#)?;
//!
//! assert_eq!(dat.games[0].name, "Some Game");
//! assert_eq!(dat.games[0].roms[0].size, 8);
//! # }
//! # Ok::<(), datary::Error>(())
//! ```
//!
//! MAME's `-listinfo` output uses the same grammar with an `emulator (` header
//! block instead of `clrmamepro (`, and is read by the same parser.
//!
//! # Round-tripping
//!
//! Unlike the XML side, which reproduces published files byte for byte, this
//! syntax has no specification and producers disagree about when to quote a
//! bare token. Round-tripping is therefore *semantic*: parsing, writing and
//! re-parsing yields an equal [`Datafile`], but the bytes may
//! differ from the input. One case is genuinely lossy: the format defines no
//! escape sequence, so a `"` inside a value cannot be represented and is
//! written as `'`.

mod ir;
mod lower;
mod parse;
mod write;

pub use ir::{Block, Entry};

use crate::dat::Datafile;
use crate::error::{Error, Result};
use crate::write::WriteOptions;

/// Parses a ClrMamePro datafile from a string.
///
/// # Errors
///
/// Returns [`Error::Cmpro`] with a line and column if the input is malformed.
pub fn from_str(source: &str) -> Result<Datafile> {
    let blocks = parse::blocks(source).map_err(Error::Cmpro)?;
    Ok(lower::to_datafile(&blocks))
}

/// Parses a ClrMamePro datafile from any reader.
///
/// The whole input is read into memory, because the parser borrows from it.
///
/// # Errors
///
/// Returns [`Error::Io`] if the reader fails, or [`Error::Cmpro`] if the input
/// is malformed.
pub fn from_reader(mut reader: impl std::io::Read) -> Result<Datafile> {
    let mut source = String::new();
    reader.read_to_string(&mut source)?;
    from_str(&source)
}

/// Reads a ClrMamePro datafile from a path.
///
/// # Errors
///
/// Returns [`Error::Io`] if the file cannot be read, or [`Error::Cmpro`] if it
/// is malformed.
pub fn read_file(path: impl AsRef<std::path::Path>) -> Result<Datafile> {
    from_str(&std::fs::read_to_string(path.as_ref())?)
}

/// Serialises a datafile in ClrMamePro syntax with the default formatting.
#[must_use]
pub fn to_string(dat: &Datafile) -> String {
    to_string_with(dat, &WriteOptions::clrmamepro())
}

/// Serialises a datafile in ClrMamePro syntax.
///
/// Honours [`WriteOptions::indent`], [`WriteOptions::line_ending`] and
/// [`WriteOptions::trailing_newline`]. [`WriteOptions::declaration`] has no
/// meaning here and is ignored.
#[must_use]
pub fn to_string_with(dat: &Datafile, options: &WriteOptions) -> String {
    write::to_string(dat, options)
}

/// Parses a datafile into its raw block tree, without interpreting the keys.
///
/// Useful for datafiles carrying fields this crate does not model.
///
/// # Errors
///
/// Returns [`Error::Cmpro`] if the input is malformed.
pub fn parse_blocks(source: &str) -> Result<Vec<Block<'_>>> {
    parse::blocks(source).map_err(Error::Cmpro)
}
