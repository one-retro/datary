//! The DAT datafile document model.
//!
//! These types are a permissive superset of three overlapping specifications:
//!
//! * the [Logiqx ROM Management Datafile DTD][logiqx] (rev 1.5), the common
//!   ancestor, used by TOSEC, MAME-derived tools and Redump;
//! * the [No-Intro datafile schema v3][nointro], which adds `<id>`, `<subset>`,
//!   game `@id`/`@cloneofid`, `<category>` and the `sha256`/`serial`/`header`
//!   ROM attributes, and drops the merge-related machinery;
//! * fields No-Intro emits in practice but never wrote into its schema, namely
//!   the ROM `mia` attribute and the `<game_id>` element used by the 3DS,
//!   Wii U and Switch datafiles.
//!
//! Anything optional in any of those is [`Option`] here, so a datafile from any
//! of the dialects parses without loss and writes back unchanged.
//!
//! [logiqx]: http://www.logiqx.com/Dats/datafile.dtd
//! [nointro]: https://datomatic.no-intro.org/stuff/schema_nointro_datfile_v3.xsd

use crate::enums::{ForceMerging, ForceNoDump, ForcePacking, RomMode, SampleMode, Status, YesNo};
use crate::hash::{Crc32, Md5, Sha1, Sha256};
use serde::{Deserialize, Serialize};

/// The `xsi` namespace URI that No-Intro datafiles declare on the root element.
pub const XSI_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// The schema location No-Intro datafiles point at.
pub const NO_INTRO_SCHEMA_LOCATION: &str =
    "https://datomatic.no-intro.org/stuff https://datomatic.no-intro.org/stuff/schema_nointro_datfile_v3.xsd";

/// A complete DAT datafile: the root `<datafile>` element.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Datafile {
    /// `@xmlns:xsi`. Present on No-Intro datafiles.
    #[serde(rename = "@xmlns:xsi", skip_serializing_if = "Option::is_none")]
    pub xmlns_xsi: Option<String>,

    /// `@xsi:schemaLocation`. Present on No-Intro datafiles.
    ///
    /// The split rename is deliberate: quick-xml strips the namespace prefix
    /// when reading attributes but writes field names verbatim, so the two
    /// directions need different spellings to round-trip.
    #[serde(
        rename(serialize = "@xsi:schemaLocation", deserialize = "@schemaLocation"),
        skip_serializing_if = "Option::is_none"
    )]
    pub schema_location: Option<String>,

    /// `@xsi:noNamespaceSchemaLocation`, an alternative spelling of the above.
    #[serde(
        rename(
            serialize = "@xsi:noNamespaceSchemaLocation",
            deserialize = "@noNamespaceSchemaLocation"
        ),
        skip_serializing_if = "Option::is_none"
    )]
    pub no_namespace_schema_location: Option<String>,

    /// `@build`. Logiqx only.
    #[serde(rename = "@build", skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,

    /// `@debug`. Logiqx only; defaults to `no`.
    #[serde(rename = "@debug", skip_serializing_if = "Option::is_none")]
    pub debug: Option<YesNo>,

    /// The `<header>` element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<Header>,

    /// The `<game>` elements.
    #[serde(rename = "game", default, skip_serializing_if = "Vec::is_empty")]
    pub games: Vec<Game>,
}

impl Datafile {
    /// Returns `true` if the datafile is flagged as a debug build.
    #[must_use]
    pub fn is_debug(&self) -> bool {
        self.debug.unwrap_or_default().is_yes()
    }

    /// Returns the first game with the given name.
    #[must_use]
    pub fn game(&self, name: &str) -> Option<&Game> {
        self.games.iter().find(|g| g.name == name)
    }

    /// Iterates over every ROM in the datafile, paired with its owning game.
    pub fn roms(&self) -> impl Iterator<Item = (&Game, &Rom)> {
        self.games
            .iter()
            .flat_map(|g| g.roms.iter().map(move |r| (g, r)))
    }

    /// Total number of ROM entries across all games.
    #[must_use]
    pub fn rom_count(&self) -> usize {
        self.games.iter().map(|g| g.roms.len()).sum()
    }

    /// Returns the clones of `name`, resolved through both `@cloneof` (Logiqx)
    /// and `@cloneofid` (No-Intro).
    #[must_use]
    pub fn clones_of<'a>(&'a self, parent: &'a Game) -> Vec<&'a Game> {
        self.games
            .iter()
            .filter(|g| {
                g.clone_of.as_deref() == Some(parent.name.as_str())
                    || (g.clone_of_id.is_some() && g.clone_of_id == parent.id)
            })
            .collect()
    }

    /// Builds the lookup tables described by [`crate::index::Index`].
    #[cfg(feature = "index")]
    #[cfg_attr(docsrs, doc(cfg(feature = "index")))]
    #[must_use]
    pub fn indexed(self) -> crate::index::IndexedDatafile {
        crate::index::IndexedDatafile::new(self)
    }
}

/// The `<header>` element: metadata describing the datafile itself.
///
/// Field order follows the intersection of the Logiqx DTD and the No-Intro
/// schema, so serialising a datafile from either dialect produces a document
/// that still validates against its original schema.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// `<id>`. The No-Intro DAT-o-MATIC system id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,

    /// `<name>`. The short name of the datafile.
    #[serde(default)]
    pub name: String,

    /// `<description>`.
    #[serde(default)]
    pub description: String,

    /// `<category>`. Logiqx/TOSEC only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// `<version>`. No-Intro uses a `YYYYMMDD-HHMMSS` stamp here.
    #[serde(default)]
    pub version: String,

    /// `<date>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,

    /// `<author>`.
    #[serde(default)]
    pub author: String,

    /// `<email>`. Logiqx/TOSEC only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// `<homepage>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    /// `<url>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// `<subset>`. No-Intro only; names a partial datafile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subset: Option<String>,

    /// `<comment>`. Logiqx/TOSEC only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    /// `<clrmamepro>`.
    #[serde(rename = "clrmamepro", skip_serializing_if = "Option::is_none")]
    pub clr_mame_pro: Option<ClrMamePro>,

    /// `<romcenter>`.
    #[serde(rename = "romcenter", skip_serializing_if = "Option::is_none")]
    pub rom_center: Option<RomCenter>,
}

/// The `<clrmamepro>` element: hints for the clrmamepro ROM manager.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClrMamePro {
    /// `@header`. Names an external header-skipper definition.
    #[serde(rename = "@header", skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,

    /// `@forcemerging`. Logiqx only; defaults to [`ForceMerging::Split`].
    #[serde(rename = "@forcemerging", skip_serializing_if = "Option::is_none")]
    pub force_merging: Option<ForceMerging>,

    /// `@forcenodump`. Defaults to [`ForceNoDump::Obsolete`].
    #[serde(rename = "@forcenodump", skip_serializing_if = "Option::is_none")]
    pub force_no_dump: Option<ForceNoDump>,

    /// `@forcepacking`. Logiqx only; defaults to [`ForcePacking::Zip`].
    #[serde(rename = "@forcepacking", skip_serializing_if = "Option::is_none")]
    pub force_packing: Option<ForcePacking>,
}

/// The `<romcenter>` element: hints for the RomCenter ROM manager.
///
/// Logiqx defines seven attributes here; the No-Intro schema keeps only
/// `@plugin`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomCenter {
    /// `@plugin`.
    #[serde(rename = "@plugin", skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,

    /// `@rommode`. Defaults to [`RomMode::Split`].
    #[serde(rename = "@rommode", skip_serializing_if = "Option::is_none")]
    pub rom_mode: Option<RomMode>,

    /// `@biosmode`. Defaults to [`RomMode::Split`].
    #[serde(rename = "@biosmode", skip_serializing_if = "Option::is_none")]
    pub bios_mode: Option<RomMode>,

    /// `@samplemode`. Defaults to [`SampleMode::Merged`].
    #[serde(rename = "@samplemode", skip_serializing_if = "Option::is_none")]
    pub sample_mode: Option<SampleMode>,

    /// `@lockrommode`. Defaults to `no`.
    #[serde(rename = "@lockrommode", skip_serializing_if = "Option::is_none")]
    pub lock_rom_mode: Option<YesNo>,

    /// `@lockbiosmode`. Defaults to `no`.
    #[serde(rename = "@lockbiosmode", skip_serializing_if = "Option::is_none")]
    pub lock_bios_mode: Option<YesNo>,

    /// `@locksamplemode`. Defaults to `no`.
    #[serde(rename = "@locksamplemode", skip_serializing_if = "Option::is_none")]
    pub lock_sample_mode: Option<YesNo>,
}

/// A `<game>` element: one titled set of ROMs.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    /// `@name`. Required; unique within the datafile.
    #[serde(rename = "@name")]
    pub name: String,

    /// `@id`. No-Intro only.
    ///
    /// Free-form text, not a number, and the schema agrees (`xs:string`).
    /// Most values are zero-padded digits — `"0001"` — but a meaningful
    /// minority carry a letter prefix marking a class of entry:
    ///
    /// | Form | Meaning | Example |
    /// | --- | --- | --- |
    /// | `0001` | An ordinary numbered entry | most of a datafile |
    /// | `z183` | Beta, proto or aftermarket | `Adventure Time … (Beta)` |
    /// | `x060` | A further out-of-band class | |
    /// | `xB01` | BIOS | `[BIOS] Nintendo 3DS ARM9 Boot ROM (World)` |
    ///
    /// Across four published No-Intro datafiles (~5,000 games), 5.6% of ids
    /// were not pure digits. Parsing this as an integer would fail outright on
    /// those, and would silently drop the zero padding on the rest.
    ///
    /// These ids are also referenced by [`Game::clone_of_id`], so the clone
    /// graph depends on them round-tripping exactly.
    #[serde(rename = "@id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// `@cloneof`. Names the parent set.
    #[serde(rename = "@cloneof", skip_serializing_if = "Option::is_none")]
    pub clone_of: Option<String>,

    /// `@cloneofid`. No-Intro only; references the parent's [`Game::id`].
    ///
    /// Free-form for the same reason [`Game::id`] is: a clone may point at a
    /// letter-prefixed parent such as `"z056"`.
    #[serde(rename = "@cloneofid", skip_serializing_if = "Option::is_none")]
    pub clone_of_id: Option<String>,

    /// `@romof`. Logiqx only.
    #[serde(rename = "@romof", skip_serializing_if = "Option::is_none")]
    pub rom_of: Option<String>,

    /// `@sampleof`. Logiqx only.
    #[serde(rename = "@sampleof", skip_serializing_if = "Option::is_none")]
    pub sample_of: Option<String>,

    /// `@sourcefile`. Logiqx only.
    #[serde(rename = "@sourcefile", skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,

    /// `@isbios`. Logiqx only; defaults to `no`.
    #[serde(rename = "@isbios", skip_serializing_if = "Option::is_none")]
    pub is_bios: Option<YesNo>,

    /// `@board`. Logiqx only.
    #[serde(rename = "@board", skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,

    /// `@rebuildto`. Logiqx only.
    #[serde(rename = "@rebuildto", skip_serializing_if = "Option::is_none")]
    pub rebuild_to: Option<String>,

    /// `<comment>` elements. Logiqx only.
    #[serde(rename = "comment", default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<String>,

    /// `<category>` elements. No-Intro only; a game may list several
    /// (for example `Games` and `Preproduction`).
    #[serde(rename = "category", default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,

    /// `<description>`. Conventionally identical to [`Game::name`].
    #[serde(default)]
    pub description: String,

    /// `<game_id>`. A No-Intro extension carrying the platform title id,
    /// used by the 3DS, Wii U and Switch datafiles. Not in any published schema.
    #[serde(rename = "game_id", skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,

    /// `<year>`. Logiqx only. Kept as a string because the DTD declares it
    /// `#PCDATA` and real datafiles contain values such as `19??`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,

    /// `<manufacturer>`. Logiqx only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,

    /// `<rom>` elements.
    #[serde(rename = "rom", default, skip_serializing_if = "Vec::is_empty")]
    pub roms: Vec<Rom>,

    /// `<release>` elements.
    #[serde(rename = "release", default, skip_serializing_if = "Vec::is_empty")]
    pub releases: Vec<Release>,

    /// `<biosset>` elements. Logiqx only.
    #[serde(rename = "biosset", default, skip_serializing_if = "Vec::is_empty")]
    pub bios_sets: Vec<BiosSet>,

    /// `<disk>` elements. Logiqx only.
    #[serde(rename = "disk", default, skip_serializing_if = "Vec::is_empty")]
    pub disks: Vec<Disk>,

    /// `<sample>` elements. Logiqx only.
    #[serde(rename = "sample", default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<Sample>,

    /// `<archive>` elements. Logiqx only.
    #[serde(rename = "archive", default, skip_serializing_if = "Vec::is_empty")]
    pub archives: Vec<Archive>,
}

impl Game {
    /// Returns `true` if this game is a BIOS set.
    #[must_use]
    pub fn is_bios(&self) -> bool {
        self.is_bios.unwrap_or_default().is_yes()
    }

    /// Returns `true` if this game is a clone of another set.
    #[must_use]
    pub fn is_clone(&self) -> bool {
        self.clone_of.is_some() || self.clone_of_id.is_some()
    }

    /// Returns `true` if any ROM in this set is flagged missing in action.
    #[must_use]
    pub fn is_mia(&self) -> bool {
        self.roms.iter().any(Rom::is_mia)
    }

    /// Returns the total size in bytes of every ROM in this set.
    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.roms.iter().map(|r| r.size).sum()
    }

    /// Returns the first ROM with the given name.
    #[must_use]
    pub fn rom(&self, name: &str) -> Option<&Rom> {
        self.roms.iter().find(|r| r.name == name)
    }

    /// Returns `true` if the game carries the given `<category>`.
    #[must_use]
    pub fn has_category(&self, category: &str) -> bool {
        self.categories.iter().any(|c| c == category)
    }
}

/// A `<rom>` element: a single file belonging to a game.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rom {
    /// `@name`. The file name, including extension.
    #[serde(rename = "@name")]
    pub name: String,

    /// `@size`, in bytes.
    ///
    /// This is a `u64`, not a `u32`: the No-Intro schema declares it
    /// `xs:unsignedInt`, but real datafiles exceed that (a decrypted 3DS entry
    /// is `4294967296`, exactly one past [`u32::MAX`]).
    #[serde(rename = "@size", default)]
    pub size: u64,

    /// `@crc`.
    #[serde(rename = "@crc", skip_serializing_if = "Option::is_none")]
    pub crc: Option<Crc32>,

    /// `@md5`.
    #[serde(rename = "@md5", skip_serializing_if = "Option::is_none")]
    pub md5: Option<Md5>,

    /// `@sha1`.
    #[serde(rename = "@sha1", skip_serializing_if = "Option::is_none")]
    pub sha1: Option<Sha1>,

    /// `@sha256`. No-Intro only.
    #[serde(rename = "@sha256", skip_serializing_if = "Option::is_none")]
    pub sha256: Option<Sha256>,

    /// `@status`. Defaults to [`Status::Good`].
    #[serde(rename = "@status", skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,

    /// `@serial`. No-Intro only; the cartridge or disc serial.
    #[serde(rename = "@serial", skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,

    /// `@header`. No-Intro only; names the header-skipper this entry needs.
    #[serde(rename = "@header", skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,

    /// `@mia`. A No-Intro extension flagging a dump nobody currently has.
    /// Emitted in practice but absent from the published schema.
    #[serde(rename = "@mia", skip_serializing_if = "Option::is_none")]
    pub mia: Option<YesNo>,

    /// `@merge`. Logiqx only; names the parent ROM this one merges into.
    #[serde(rename = "@merge", skip_serializing_if = "Option::is_none")]
    pub merge: Option<String>,

    /// `@date`. Logiqx only.
    #[serde(rename = "@date", skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

impl Rom {
    /// Returns the dump status, applying the DTD default of [`Status::Good`].
    #[must_use]
    pub fn status(&self) -> Status {
        self.status.unwrap_or_default()
    }

    /// Returns `true` if this ROM is flagged missing in action.
    #[must_use]
    pub fn is_mia(&self) -> bool {
        self.mia.unwrap_or_default().is_yes()
    }

    /// Returns `true` if no correct dump of this ROM is known to exist.
    #[must_use]
    pub fn is_no_dump(&self) -> bool {
        self.status() == Status::NoDump
    }

    /// Returns `true` if the entry carries at least one checksum.
    #[must_use]
    pub fn has_checksum(&self) -> bool {
        self.crc.is_some() || self.md5.is_some() || self.sha1.is_some() || self.sha256.is_some()
    }

    /// Returns the file extension of [`Rom::name`], lowercased.
    #[must_use]
    pub fn extension(&self) -> Option<String> {
        std::path::Path::new(&self.name)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
    }
}

/// A `<release>` element: a regional release of a game.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    /// `@name`.
    #[serde(rename = "@name")]
    pub name: String,

    /// `@region`, such as `EU`, `JP` or `USA`.
    #[serde(rename = "@region", default)]
    pub region: String,

    /// `@language`. Logiqx only.
    #[serde(rename = "@language", skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// `@date`. Logiqx only.
    #[serde(rename = "@date", skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,

    /// `@default`. Logiqx only; defaults to `no`.
    #[serde(rename = "@default", skip_serializing_if = "Option::is_none")]
    pub default: Option<YesNo>,
}

impl Release {
    /// Returns `true` if this is the default release for the game.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.default.unwrap_or_default().is_yes()
    }
}

/// A `<biosset>` element. Logiqx only.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiosSet {
    /// `@name`.
    #[serde(rename = "@name")]
    pub name: String,

    /// `@description`.
    #[serde(rename = "@description", default)]
    pub description: String,

    /// `@default`; defaults to `no`.
    #[serde(rename = "@default", skip_serializing_if = "Option::is_none")]
    pub default: Option<YesNo>,
}

impl BiosSet {
    /// Returns `true` if this is the default BIOS for the game.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.default.unwrap_or_default().is_yes()
    }
}

/// A `<disk>` element: a CHD or similar disk image. Logiqx only.
///
/// Unlike [`Rom`], a disk has no size or CRC in the DTD.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disk {
    /// `@name`.
    #[serde(rename = "@name")]
    pub name: String,

    /// `@md5`.
    #[serde(rename = "@md5", skip_serializing_if = "Option::is_none")]
    pub md5: Option<Md5>,

    /// `@sha1`.
    #[serde(rename = "@sha1", skip_serializing_if = "Option::is_none")]
    pub sha1: Option<Sha1>,

    /// `@merge`.
    #[serde(rename = "@merge", skip_serializing_if = "Option::is_none")]
    pub merge: Option<String>,

    /// `@status`. Defaults to [`Status::Good`].
    #[serde(rename = "@status", skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
}

impl Disk {
    /// Returns the dump status, applying the DTD default of [`Status::Good`].
    #[must_use]
    pub fn status(&self) -> Status {
        self.status.unwrap_or_default()
    }
}

/// A `<sample>` element. Logiqx only.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    /// `@name`.
    #[serde(rename = "@name")]
    pub name: String,
}

/// An `<archive>` element. Logiqx only.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Archive {
    /// `@name`.
    #[serde(rename = "@name")]
    pub name: String,
}
