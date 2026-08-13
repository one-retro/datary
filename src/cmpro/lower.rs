//! Mapping between the block tree and [`Datafile`].
//!
//! Every dialect quirk lives here rather than in the grammar.

use super::ir::Block;
use crate::dat::{Archive, ClrMamePro, Datafile, Disk, Game, Header, Rom, Sample};
use crate::enums::{ForceMerging, ForceNoDump, ForcePacking, Status};
use crate::error::{CmproError, Error, Position, Result};
use crate::hash::HashParseError;
use std::str::FromStr;

/// Parses a checksum, naming the offending key and value on failure.
///
/// An optional `0x` prefix is accepted: ClrMamePro and ckmame both emit it on
/// occasion (`crc32 0xd87f7e0c`), though most values are bare. Sizes are always
/// plain decimal and are not treated this way.
///
/// Deliberately *not* lenient about anything else: silently dropping a
/// malformed checksum would leave the entry with no checksum at all, and
/// `Rom::verify` does not check what an entry does not record — so a typo would
/// turn into a ROM that verifies against anything of the right size.
fn hash<T>(block: &Block<'_>, keys: &[&str], source: &str) -> Result<Option<T>>
where
    T: FromStr<Err = HashParseError>,
{
    let Some(field) = block.any_field(keys) else {
        return Ok(None);
    };
    let raw = field
        .strip_prefix("0x")
        .or_else(|| field.strip_prefix("0X"))
        .unwrap_or(field);
    let prefix = field.len() - raw.len();

    match raw.parse::<T>() {
        Ok(value) => Ok(Some(value)),
        Err(cause) => {
            // A bad digit is pinned to that digit; a bad length is about the
            // whole value, so it stays at the start.
            let within = match cause {
                HashParseError::InvalidDigit { index, .. } => prefix + index,
                HashParseError::InvalidLength { .. } => 0,
            };

            // The value is a slice of the source, so its position is
            // recoverable without the parser having tracked a span.
            let position = Position::of_substring(source, field)
                .map(|start| Position::from_offset(source, start.offset + within));

            Err(Error::Cmpro(CmproError {
                message: format!("invalid {} value {field:?}: {cause}", keys[0]),
                position,
            }))
        }
    }
}

/// Header block names. ClrMamePro writes `clrmamepro`; MAME `-listinfo` writes
/// `emulator` with otherwise identical structure.
const HEADER_BLOCKS: [&str; 2] = ["clrmamepro", "emulator"];

/// Set block names. `resource` marks a BIOS/support set in some datafiles.
const GAME_BLOCKS: [&str; 4] = ["game", "machine", "resource", "set"];

/// Builds a [`Datafile`] from parsed blocks.
///
/// # Errors
///
/// Returns [`Error::Cmpro`] if a checksum value is not valid hex of the width
/// its algorithm requires.
pub fn to_datafile(blocks: &[Block<'_>], source: &str) -> Result<Datafile> {
    let mut dat = Datafile::default();

    for block in blocks {
        if HEADER_BLOCKS.contains(&block.name) {
            dat.header = Some(to_header(block));
        } else if GAME_BLOCKS.contains(&block.name) {
            let mut game = to_game(block, source)?;
            // `resource` sets are BIOS-like; record that in the model.
            if block.name == "resource" {
                game.is_bios = Some(crate::enums::YesNo::Yes);
            }
            dat.games.push(game);
        }
    }

    Ok(dat)
}

fn to_header(block: &Block<'_>) -> Header {
    let clr_mame_pro = ClrMamePro {
        header: block.field("header").map(str::to_string),
        force_merging: block.field("forcemerging").and_then(parse_force_merging),
        force_no_dump: block.field("forcenodump").and_then(parse_force_no_dump),
        // `forcepacking` is the real key, in both Logiqx's CMPro example and
        // ckmame's parser. `forcezipping` appears only as a proposed tool flag
        // in a 2007 DatUtil note, but is accepted on read in case it escaped.
        force_packing: block
            .any_field(&["forcepacking", "forcezipping"])
            .and_then(parse_force_packing),
    };

    Header {
        name: block.field("name").unwrap_or_default().to_string(),
        description: block.field("description").unwrap_or_default().to_string(),
        version: block.field("version").unwrap_or_default().to_string(),
        author: block.field("author").unwrap_or_default().to_string(),
        category: block.field("category").map(str::to_string),
        comment: block.field("comment").map(str::to_string),
        date: block.field("date").map(str::to_string),
        email: block.field("email").map(str::to_string),
        homepage: block.field("homepage").map(str::to_string),
        url: block.field("url").map(str::to_string),
        clr_mame_pro: (clr_mame_pro != ClrMamePro::default()).then_some(clr_mame_pro),
        ..Header::default()
    }
}

fn to_game(block: &Block<'_>, source: &str) -> Result<Game> {
    let mut game = Game {
        name: block.field("name").unwrap_or_default().to_string(),
        description: block.field("description").unwrap_or_default().to_string(),
        year: block.field("year").map(str::to_string),
        manufacturer: block.field("manufacturer").map(str::to_string),
        clone_of: block.field("cloneof").map(str::to_string),
        rom_of: block.field("romof").map(str::to_string),
        sample_of: block.field("sampleof").map(str::to_string),
        source_file: block.field("sourcefile").map(str::to_string),
        ..Game::default()
    };

    for rom in block.blocks("rom") {
        game.roms.push(Rom {
            name: rom.field("name").unwrap_or_default().to_string(),
            size: rom.field("size").and_then(|s| s.parse().ok()).unwrap_or(0),
            // ckmame writes `crc32`, ClrMamePro writes `crc`.
            crc: hash(rom, &["crc", "crc32"], source)?,
            md5: hash(rom, &["md5"], source)?,
            sha1: hash(rom, &["sha1"], source)?,
            sha256: hash(rom, &["sha256"], source)?,
            merge: rom.field("merge").map(str::to_string),
            date: rom.field("date").map(str::to_string),
            // The XML schema puts `serial` on the rom, and this crate writes it
            // there. Whether real ClrMamePro exports do the same is unverified,
            // so a serial on the enclosing game block is inherited by any rom
            // that lacks its own. That way either convention reads correctly.
            serial: rom
                .field("serial")
                .or_else(|| block.field("serial"))
                .map(str::to_string),
            status: rom.any_field(&["flags", "status"]).and_then(parse_status),
            ..Rom::default()
        });
    }

    for disk in block.blocks("disk") {
        game.disks.push(Disk {
            name: disk.field("name").unwrap_or_default().to_string(),
            md5: hash(disk, &["md5"], source)?,
            sha1: hash(disk, &["sha1"], source)?,
            merge: disk.field("merge").map(str::to_string),
            status: disk.any_field(&["flags", "status"]).and_then(parse_status),
        });
    }

    // `sample` is a bare scalar in this syntax — `sample shot.wav`, repeated
    // once per sample — not a block. ckmame reads it that way and so does
    // ClrMamePro's own documented example. The block spelling is accepted too,
    // since this crate wrote it before that was understood.
    for name in block.fields("sample") {
        game.samples.push(Sample {
            name: name.to_string(),
        });
    }
    for sample in block.blocks("sample") {
        game.samples.push(Sample {
            name: sample.field("name").unwrap_or_default().to_string(),
        });
    }

    // `archive` is a stub in ckmame's parser, which consumes no value, so its
    // shape is genuinely unsettled. Both spellings are accepted rather than
    // betting on one.
    for name in block.fields("archive") {
        game.archives.push(Archive {
            name: name.to_string(),
        });
    }
    for archive in block.blocks("archive") {
        game.archives.push(Archive {
            name: archive.field("name").unwrap_or_default().to_string(),
        });
    }

    Ok(game)
}

fn parse_status(s: &str) -> Option<Status> {
    match s {
        "baddump" | "bad" => Some(Status::BadDump),
        "nodump" => Some(Status::NoDump),
        "verified" | "good" => Some(if s == "verified" {
            Status::Verified
        } else {
            Status::Good
        }),
        _ => None,
    }
}

fn parse_force_merging(s: &str) -> Option<ForceMerging> {
    match s {
        "none" => Some(ForceMerging::None),
        "split" => Some(ForceMerging::Split),
        "full" => Some(ForceMerging::Full),
        _ => None,
    }
}

fn parse_force_no_dump(s: &str) -> Option<ForceNoDump> {
    match s {
        "obsolete" => Some(ForceNoDump::Obsolete),
        "required" => Some(ForceNoDump::Required),
        "ignore" => Some(ForceNoDump::Ignore),
        _ => None,
    }
}

fn parse_force_packing(s: &str) -> Option<ForcePacking> {
    match s {
        "zip" => Some(ForcePacking::Zip),
        "unzip" => Some(ForcePacking::Unzip),
        _ => None,
    }
}
