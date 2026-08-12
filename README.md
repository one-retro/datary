# datary

[![Crates.io](https://img.shields.io/crates/v/datary.svg)](https://crates.io/crates/datary)
[![Documentation](https://docs.rs/datary/badge.svg)](https://docs.rs/datary)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Read and write the DAT datafiles published by [No-Intro], [Redump], [TOSEC] and
other ROM preservation projects — fully modelled in Rust's type system.

Every enumerated attribute becomes a Rust enum and every checksum becomes a
fixed-size value, so a datafile is validated once when it is parsed instead of
being carried around as a bag of strings.

```toml
[dependencies]
datary = "0.2"
```

## Reading

```rust
let dat = datary::read_file("Nintendo - Virtual Boy.dat")?;

for game in &dat.games {
    for rom in &game.roms {
        println!("{} ({} bytes) {}", rom.name, rom.size, rom.sha1.unwrap());
    }
}
```

## Checksums are real types

Checksums are reused from the [RustCrypto] hash crates rather than kept as
strings. A value read from a datafile is therefore *the same type* as one
computed over a file on disk, and the two compare directly — no case
normalisation, no length guessing:

```rust
use datary::hash::Sha1;

let rom = &dat.games[0].roms[0];
if rom.sha1 == Some(Sha1::compute(&std::fs::read("game.vb")?)) {
    println!("verified");
}
```

`Crc32`, `Md5`, `Sha1` and `Sha256` all parse case-insensitively, reject
malformed input at parse time, and write back as canonical lowercase hex.

## Enumerations are real enums

```rust
use datary::{Status, YesNo};

assert_eq!(rom.status(), Status::NoDump);   // absent means Good, per the DTD
assert_eq!(rom.mia, Some(YesNo::Yes));
assert!(game.has_category("Preproduction"));
```

## Verifying a collection

```rust
use datary::verify::FileHashes;

let dat = datary::read_file("Nintendo - Virtual Boy.dat")?.indexed();
let hashes = FileHashes::of_file("3-D Tetris (USA).vb")?;  // one pass, all checksums

match dat.find(&hashes) {
    Some((game, _rom)) => println!("recognised: {}", game.name),
    None => println!("not in the datafile"),
}
```

`find` looks up by the strongest checksum available — SHA-256, then SHA-1, MD5,
then CRC32 — and confirms the candidate agrees on every *other* checksum the
entry records, so a CRC collision is not mistaken for a match.

When a file is close but not right, `Rom::verify` reports exactly which
checksums disagreed rather than a bare boolean:

```rust
if let Err(problems) = rom.verify(&hashes) {
    for problem in problems {
        println!("{problem}");  // "sha1 mismatch: expected 5177…, got aaf4…"
    }
}
```

## Writing

Output is byte-for-byte identical to the files DAT-o-MATIC publishes:

```rust
use datary::WriteOptions;

datary::write_file_with("out.dat", &dat, &WriteOptions::no_intro())?;
```

`WriteOptions` also covers line endings, indentation, the XML declaration and
the trailing newline; `WriteOptions::default()` produces conventional LF output
and `WriteOptions::compact()` produces a single line.

## Which DAT dialect?

One set of types covers all of them. `datary` models the union of:

- the **[Logiqx DTD]** (rev 1.5) — the common ancestor, used by TOSEC, Redump
  and MAME-derived tools: `<biosset>`, `<disk>`, `<sample>`, `<archive>`,
  `<year>`, `<manufacturer>`, merge attributes and the `romcenter` modes;
- the **[No-Intro schema v3]** — adds `<id>`, `<subset>`, `<category>`, game
  `@id`/`@cloneofid` and the `sha256`/`serial`/`header` ROM attributes;
- **what No-Intro actually emits** but never wrote into its schema — the ROM
  `mia` attribute, and the `<game_id>` element used by the 3DS, Wii U and
  Switch datafiles.

Everything optional in any of those is an `Option` here, so files from any
dialect parse without loss and write back unchanged. Unknown elements and
attributes are ignored rather than rejected, so a future No-Intro extension
will not break parsing.

Two places where the published schema does not match reality, and `datary`
follows reality:

| Field | Schema says | Reality |
| --- | --- | --- |
| `rom/@size` | `xs:unsignedInt` | A decrypted 3DS entry is `4294967296` — one past `u32::MAX`. Modelled as `u64`. |
| `game/@id` | `xs:string` | Zero-padded (`"0001"`). Kept as a string so the padding survives a round trip. |

## Feature flags

| Feature | Default | What it adds |
| --- | --- | --- |
| `index` | yes | `IndexedDatafile`: lookup by checksum, size, game name/id, or ROM name prefix |
| `verify` | yes | `FileHashes` and `Rom::verify` for checking files on disk |

Both can be disabled for a parse-and-serialise-only build.

## Examples

```sh
cargo run --example info -- tests/fixtures/no-intro/virtual-boy.dat
cargo run --example scan -- Nintendo\ -\ Virtual\ Boy.dat ~/roms/vb
```

## Testing

The test suite runs against real datafiles published by No-Intro and TOSEC,
plus a handcrafted file exercising the corners of the Logiqx DTD that neither
project emits. The strongest check reparses each published No-Intro file and
asserts the re-serialised output is byte-identical to the original.

```sh
cargo test
```

## License

Apache-2.0. See [LICENSE](LICENSE).

[No-Intro]: https://no-intro.org
[Redump]: http://redump.org
[TOSEC]: https://www.tosecdev.org
[RustCrypto]: https://github.com/RustCrypto/hashes
[Logiqx DTD]: http://www.logiqx.com/Dats/datafile.dtd
[No-Intro schema v3]: https://datomatic.no-intro.org/stuff/schema_nointro_datfile_v3.xsd
