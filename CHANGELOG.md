# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-08-12

Moved out of the `retronomicon` monorepo into its own repository, updated to the
current No-Intro datafile format, and given a real test suite.

### Added

- **No-Intro schema v3 support**: header `<id>` and `<subset>`, game `@id`,
  `@cloneofid` and `<category>` (repeatable), and the `sha256`, `serial` and
  `header` ROM attributes.
- **Fields No-Intro emits but never published in its schema**: the ROM `mia`
  attribute, and the `<game_id>` element used by the 3DS, Wii U and Switch
  datafiles.
- **Missing Logiqx DTD coverage**: game `@sourcefile`, and `RomCenter` is now
  complete.
- **Typed checksums** (`hash` module): `Crc32`, `Md5`, `Sha1` and `Sha256`.
  The digest types reuse [RustCrypto]'s `digest::Output<D>`, so a checksum read
  from a datafile is the same type as one computed over a file and the two
  compare directly. Parsing is case-insensitive and validates width; writing
  emits canonical lowercase hex.
- **`verify` feature**: `FileHashes` computes every checksum in one pass over a
  file, `Rom::verify` reports exactly which checksums disagreed, and
  `IndexedDatafile::find` resolves a hashed file to its entry using the
  strongest checksum available.
- **`WriteOptions`**: control over line endings, indentation, XML declaration
  and trailing newline. `WriteOptions::no_intro()` reproduces published
  No-Intro datafiles byte for byte.
- Helper methods across the model: `Datafile::roms`, `rom_count`, `clones_of`,
  `Game::is_bios`, `is_clone`, `is_mia`, `total_size`, `has_category`,
  `Rom::status`, `is_mia`, `is_no_dump`, `has_checksum`, `extension`, and more.
- `from_str`, `write_file`, and `to_writer` over `std::io::Write`.
- Test suite covering real published No-Intro and TOSEC datafiles, a
  handcrafted file exercising the full Logiqx DTD, malformed input, and
  byte-exact reproduction of every No-Intro fixture.
- Examples: `info` and `scan`.

### Fixed

- **Game `@id` was parsed as `u32`**, which discarded No-Intro's zero padding
  (`"0001"` became `1` and was written back as `1`). It is now a `String`, as
  the schema specifies.
- **ROM `@size` was `usize`**, which overflows on 32-bit targets: real
  datafiles contain `4294967296`, one past `u32::MAX`. It is now `u64`.
- **`Release` and `BiosSet` fields were private**, making both types unusable
  from outside the crate. All model fields are now public.
- **`Datafile::debug` was a `bool`**, so parsing failed on the DTD's `debug="no"`.
  It is now `Option<YesNo>`.
- Optional attributes with spec-defined defaults are now `Option`, so a value
  absent from the input is not invented on output. Accessor methods
  (`Rom::status()`, `Game::is_bios()`, …) apply the documented default.
- The ROM-name lookup no longer asserts that names are unique; duplicates
  across games are normal and are now all retained.

### Changed

- **Breaking**: checksum fields on `Rom` and `Disk` changed from
  `Option<String>` to typed checksum values.
- **Breaking**: the `optimized` feature and `OptimizedDatafile` are replaced by
  the `index` feature and `IndexedDatafile`. The `ouroboros` self-referencing
  dependency is gone; the index now addresses entries by `RomRef` index pairs,
  making it `Send`, `Sync` and movable. `IndexedDatafile` dereferences to
  `Datafile`, and `modify` rebuilds the index after a mutation.
- **Breaking**: the ten near-duplicate yes/no enums (`IsBios`, `LockRomMode`,
  `LockBiosMode`, `LockSampleMode`, and an enum named `Default` that shadowed
  `std::default::Default`) are consolidated into a single `YesNo`.
- **Breaking**: `ForceMerge` → `ForceMerging`, `ForcePack` → `ForcePacking`,
  and `BiosMode` is folded into `RomMode` — they had identical variants.
- **Breaking**: `Datafile::parse` is replaced by the crate-level `from_reader`,
  `from_str` and `read_file` functions.
- **Breaking**: `to_writer` now takes a `std::io::Write` rather than a
  `std::fmt::Write`, and output includes an XML declaration.
- `Error` is `#[non_exhaustive]` and gained a `Serialize` variant (quick-xml
  splits serialisation and deserialisation errors) and a `Hash` variant.
- Unknown elements and attributes are ignored rather than rejected, so future
  format extensions will not break parsing.
- Minimum supported Rust version is 1.85, set by the RustCrypto 0.11 crates.

## [0.1.0]

Initial release, as part of the [retronomicon] repository.

[RustCrypto]: https://github.com/RustCrypto/hashes
[retronomicon]: https://github.com/one-retro/retronomicon
