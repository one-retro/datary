//! Lookup tables over a parsed datafile.
//!
//! Scanning a directory of ROMs against a datafile means asking "which entry
//! has this SHA-1?" once per file. [`Index`] precomputes the tables that make
//! each of those a hash lookup instead of a linear scan.
//!
//! Entries are addressed by [`RomRef`], a pair of indices into
//! [`Datafile::games`] and [`Game::roms`], rather than by borrowed references.
//! That keeps the index a plain owned value: it is [`Send`], [`Sync`] and
//! movable, and it can be built, stored and passed around independently of the
//! datafile it describes.
//!
//! ```
//! use datary::hash::Sha1;
//! use std::str::FromStr;
//!
//! let dat = datary::from_str(r#"
//!     <datafile><game name="Some Game"><description>Some Game</description>
//!         <rom name="some.rom" size="8" sha1="aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"/>
//!     </game></datafile>"#)?.indexed();
//!
//! let sha1 = Sha1::from_str("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d").unwrap();
//! assert_eq!(dat.game_by_sha1(&sha1).unwrap().name, "Some Game");
//! # Ok::<(), datary::Error>(())
//! ```

use crate::dat::{Datafile, Game, Rom};
use crate::hash::{Crc32, Md5, Sha1, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;

/// The address of a single ROM entry within a [`Datafile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RomRef {
    /// Index into [`Datafile::games`].
    pub game: usize,
    /// Index into [`Game::roms`] of that game.
    pub rom: usize,
}

const NO_REFS: &[RomRef] = &[];

/// Lookup tables built from a [`Datafile`].
///
/// Built by [`Index::build`], or implicitly by [`IndexedDatafile::new`].
/// The index is only valid for the datafile it was built from; mutating that
/// datafile's games or ROMs invalidates it.
#[derive(Debug, Clone, Default)]
pub struct Index {
    crc: HashMap<Crc32, Vec<RomRef>>,
    md5: HashMap<Md5, Vec<RomRef>>,
    sha1: HashMap<Sha1, Vec<RomRef>>,
    sha256: HashMap<Sha256, Vec<RomRef>>,
    size: HashMap<u64, Vec<RomRef>>,
    rom_names: BTreeMap<String, Vec<RomRef>>,
    game_names: HashMap<String, usize>,
    game_ids: HashMap<String, usize>,
}

impl Index {
    /// Builds the lookup tables for `datafile`.
    #[must_use]
    pub fn build(datafile: &Datafile) -> Self {
        let mut index = Self::default();

        for (game_idx, game) in datafile.games.iter().enumerate() {
            index.game_names.insert(game.name.clone(), game_idx);
            if let Some(id) = &game.id {
                index.game_ids.insert(id.clone(), game_idx);
            }

            for (rom_idx, rom) in game.roms.iter().enumerate() {
                let r = RomRef {
                    game: game_idx,
                    rom: rom_idx,
                };

                if let Some(crc) = rom.crc {
                    index.crc.entry(crc).or_default().push(r);
                }
                if let Some(md5) = rom.md5 {
                    index.md5.entry(md5).or_default().push(r);
                }
                if let Some(sha1) = rom.sha1 {
                    index.sha1.entry(sha1).or_default().push(r);
                }
                if let Some(sha256) = rom.sha256 {
                    index.sha256.entry(sha256).or_default().push(r);
                }
                index.size.entry(rom.size).or_default().push(r);
                index.rom_names.entry(rom.name.clone()).or_default().push(r);
            }
        }

        index
    }

    /// ROM entries with the given CRC32.
    #[must_use]
    pub fn by_crc(&self, crc: Crc32) -> &[RomRef] {
        self.crc.get(&crc).map_or(NO_REFS, Vec::as_slice)
    }

    /// ROM entries with the given MD5.
    #[must_use]
    pub fn by_md5(&self, md5: &Md5) -> &[RomRef] {
        self.md5.get(md5).map_or(NO_REFS, Vec::as_slice)
    }

    /// ROM entries with the given SHA-1.
    #[must_use]
    pub fn by_sha1(&self, sha1: &Sha1) -> &[RomRef] {
        self.sha1.get(sha1).map_or(NO_REFS, Vec::as_slice)
    }

    /// ROM entries with the given SHA-256.
    #[must_use]
    pub fn by_sha256(&self, sha256: &Sha256) -> &[RomRef] {
        self.sha256.get(sha256).map_or(NO_REFS, Vec::as_slice)
    }

    /// ROM entries of exactly the given size in bytes.
    #[must_use]
    pub fn by_size(&self, size: u64) -> &[RomRef] {
        self.size.get(&size).map_or(NO_REFS, Vec::as_slice)
    }

    /// ROM entries with exactly the given file name.
    #[must_use]
    pub fn by_rom_name(&self, name: &str) -> &[RomRef] {
        self.rom_names.get(name).map_or(NO_REFS, Vec::as_slice)
    }

    /// ROM entries whose file name starts with `prefix`, in name order.
    pub fn by_rom_name_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = RomRef> + 'a {
        self.rom_names
            .range(prefix.to_owned()..)
            .take_while(move |(name, _)| name.starts_with(prefix))
            .flat_map(|(_, refs)| refs.iter().copied())
    }

    /// Index into [`Datafile::games`] of the game with the given `@name`.
    #[must_use]
    pub fn game_by_name(&self, name: &str) -> Option<usize> {
        self.game_names.get(name).copied()
    }

    /// Index into [`Datafile::games`] of the game with the given `@id`.
    #[must_use]
    pub fn game_by_id(&self, id: &str) -> Option<usize> {
        self.game_ids.get(id).copied()
    }

    /// Total number of ROM entries covered by the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rom_names.values().map(Vec::len).sum()
    }

    /// Returns `true` if the index covers no ROM entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rom_names.is_empty()
    }
}

/// A [`Datafile`] bundled with its [`Index`].
///
/// Dereferences to the underlying [`Datafile`], so all of its fields and
/// methods remain available:
///
/// ```
/// # let dat = datary::from_str("<datafile><game name=\"g\"><description>g</description></game></datafile>")?.indexed();
/// assert_eq!(dat.games.len(), 1); // via Deref
/// # Ok::<(), datary::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct IndexedDatafile {
    datafile: Datafile,
    index: Index,
}

impl IndexedDatafile {
    /// Takes ownership of `datafile` and builds its index.
    #[must_use]
    pub fn new(datafile: Datafile) -> Self {
        let index = Index::build(&datafile);
        Self { datafile, index }
    }

    /// The underlying datafile.
    #[must_use]
    pub fn datafile(&self) -> &Datafile {
        &self.datafile
    }

    /// The lookup tables.
    #[must_use]
    pub fn index(&self) -> &Index {
        &self.index
    }

    /// Discards the index and returns the datafile.
    #[must_use]
    pub fn into_inner(self) -> Datafile {
        self.datafile
    }

    /// Modifies the datafile and rebuilds the index afterwards.
    pub fn modify(&mut self, f: impl FnOnce(&mut Datafile)) {
        f(&mut self.datafile);
        self.index = Index::build(&self.datafile);
    }

    /// Resolves a [`RomRef`] to its game and ROM.
    ///
    /// # Panics
    ///
    /// Panics if `r` did not come from this datafile's index.
    #[must_use]
    pub fn resolve(&self, r: RomRef) -> (&Game, &Rom) {
        let game = &self.datafile.games[r.game];
        (game, &game.roms[r.rom])
    }

    fn games_of<'a>(&'a self, refs: &'a [RomRef]) -> impl Iterator<Item = &'a Game> + 'a {
        let mut last = None;
        refs.iter().filter_map(move |r| {
            // Refs for one key arrive in game order, so a single-element memo
            // is enough to avoid yielding the same game twice.
            if last == Some(r.game) {
                return None;
            }
            last = Some(r.game);
            Some(&self.datafile.games[r.game])
        })
    }

    /// The first game containing a ROM with the given SHA-1.
    #[must_use]
    pub fn game_by_sha1(&self, sha1: &Sha1) -> Option<&Game> {
        self.games_by_sha1(sha1).next()
    }

    /// Every game containing a ROM with the given SHA-1.
    pub fn games_by_sha1<'a>(&'a self, sha1: &Sha1) -> impl Iterator<Item = &'a Game> + 'a {
        self.games_of(self.index.by_sha1(sha1))
    }

    /// The first game containing a ROM with the given MD5.
    #[must_use]
    pub fn game_by_md5(&self, md5: &Md5) -> Option<&Game> {
        self.games_by_md5(md5).next()
    }

    /// Every game containing a ROM with the given MD5.
    pub fn games_by_md5<'a>(&'a self, md5: &Md5) -> impl Iterator<Item = &'a Game> + 'a {
        self.games_of(self.index.by_md5(md5))
    }

    /// The first game containing a ROM with the given CRC32.
    #[must_use]
    pub fn game_by_crc(&self, crc: Crc32) -> Option<&Game> {
        self.games_by_crc(crc).next()
    }

    /// Every game containing a ROM with the given CRC32.
    pub fn games_by_crc(&self, crc: Crc32) -> impl Iterator<Item = &Game> + '_ {
        self.games_of(self.index.by_crc(crc))
    }

    /// The first game containing a ROM with the given SHA-256.
    #[must_use]
    pub fn game_by_sha256(&self, sha256: &Sha256) -> Option<&Game> {
        self.games_by_sha256(sha256).next()
    }

    /// Every game containing a ROM with the given SHA-256.
    pub fn games_by_sha256<'a>(&'a self, sha256: &Sha256) -> impl Iterator<Item = &'a Game> + 'a {
        self.games_of(self.index.by_sha256(sha256))
    }

    /// Every game containing a ROM of exactly the given size.
    pub fn games_by_size(&self, size: u64) -> impl Iterator<Item = &Game> + '_ {
        self.games_of(self.index.by_size(size))
    }

    /// The game with the given `@name`.
    #[must_use]
    pub fn game_by_name(&self, name: &str) -> Option<&Game> {
        self.index
            .game_by_name(name)
            .map(|i| &self.datafile.games[i])
    }

    /// The game with the given No-Intro `@id`.
    #[must_use]
    pub fn game_by_id(&self, id: &str) -> Option<&Game> {
        self.index.game_by_id(id).map(|i| &self.datafile.games[i])
    }

    /// Every game/ROM pair whose ROM file name starts with `prefix`.
    pub fn by_rom_name_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = (&'a Game, &'a Rom)> + 'a {
        self.index
            .by_rom_name_prefix(prefix)
            .map(move |r| self.resolve(r))
    }

    /// The game/ROM pair for the first ROM with exactly the given file name.
    #[must_use]
    pub fn by_rom_name(&self, name: &str) -> Option<(&Game, &Rom)> {
        self.index
            .by_rom_name(name)
            .first()
            .map(|&r| self.resolve(r))
    }
}

impl Deref for IndexedDatafile {
    type Target = Datafile;

    fn deref(&self) -> &Self::Target {
        &self.datafile
    }
}

impl From<Datafile> for IndexedDatafile {
    fn from(datafile: Datafile) -> Self {
        Self::new(datafile)
    }
}

impl From<IndexedDatafile> for Datafile {
    fn from(indexed: IndexedDatafile) -> Self {
        indexed.datafile
    }
}
