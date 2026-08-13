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
| `game/@id` | `xs:string` | Genuinely free-form — 5.6% of ~5,000 ids sampled from published datafiles are letter-prefixed (`z183` beta, `xB01` BIOS), and the rest are zero-padded. An integer would fail on the former and lose the padding on the latter. |

## Two syntaxes, one model

Projects publish datafiles both as Logiqx XML and in ClrMamePro's native
brace-delimited syntax. `datary` reads both into the same `Datafile`, so
everything downstream works regardless of which arrived:

```rust
use datary::format::{ClrMamePro, DatFormat, Xml};

// Both conventionally use a .dat extension, so read_file sniffs the content.
let dat = datary::read_file("Nintendo - Virtual Boy.dat")?;

// ...and a file read in one syntax can be written in the other.
let text = ClrMamePro.write(&dat, &datary::WriteOptions::clrmamepro())?;
```

MAME's `-listinfo` output uses the same grammar with an `emulator (` header
and is read by the same parser.

Formats are a public trait, so a downstream crate can add one and use the same
reading, writing and detection helpers:

```rust
use datary::format::{detect, DatFormat};

struct Sfv;
impl DatFormat for Sfv {
    fn name(&self) -> &'static str { "SFV" }
    fn detect(&self, bytes: &[u8]) -> bool { bytes.starts_with(b"; ") }
    fn parse(&self, source: &str) -> datary::Result<datary::Datafile> { /* ... */ }
    fn write(&self, dat: &datary::Datafile, o: &datary::WriteOptions) -> datary::Result<String> { /* ... */ }
}

let dat = datary::from_str_as(source, &Sfv)?;
let format = detect(bytes, &[&Sfv, &Xml]).unwrap();
```

One caveat: the ClrMamePro syntax has no specification, and producers disagree
about when to quote a bare token, so round-tripping it is *semantic* rather than
byte-exact. Values themselves survive intact — a backslash escapes the next
character, so quotes and backslashes are written out rather than mangled. The
XML side stays byte-exact.

## Checking a datafile

Parsing is permissive by design — a `cloneof` naming a game that is not in the
file is still well-formed, and still useful for everything that does not follow
that link. Referential integrity is therefore reported separately, and never
fails a parse:

```rust
for issue in dat.validate() {
    println!("{issue}");
}
// game "Dup": name "Dup" is already used by game #0
// game "Child": cloneof names an unknown game "Nowhere"
// game "A": clone cycle: A -> B -> A
```

It checks that `cloneof`, `cloneofid`, `romof`, `sampleof` and `merge` all
resolve, that game names and ids are unique (they are the keys those references
resolve through), and that the clone graph is acyclic. It deliberately does not
police conventions such as a `description` differing from a `name`, because
published datafiles do that on purpose.

```sh
cargo run --example lint -- Nintendo\ -\ Virtual\ Boy.dat
```

## Feature flags

| Feature | Default | What it adds |
| --- | --- | --- |
| `index` | yes | `IndexedDatafile`: lookup by checksum, size, game name/id, or ROM name prefix |
| `verify` | yes | `FileHashes` and `Rom::verify` for checking files on disk |
| `cmpro` | yes | Reading and writing the native ClrMamePro syntax |

All can be disabled for an XML-only build.

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

## Development

A [`justfile`](justfile) collects the common tasks — run `just` to list them.

```sh
just test          # cargo test --all-features
just ci            # everything CI runs: fmt, clippy, test, features, doc, msrv
just info          # summarise a bundled fixture
just doc --open    # build and read the docs
```

`just ci` mirrors the GitHub Actions workflow, so a green run locally means a
green run upstream. The MSRV it checks is read from `Cargo.toml` rather than
duplicated, so the two cannot drift apart.

## License

Apache-2.0. See [LICENSE](LICENSE).

[No-Intro]: https://no-intro.org
[Redump]: http://redump.org
[TOSEC]: https://www.tosecdev.org
[RustCrypto]: https://github.com/RustCrypto/hashes
[Logiqx DTD]: http://www.logiqx.com/Dats/datafile.dtd
[No-Intro schema v3]: https://datomatic.no-intro.org/stuff/schema_nointro_datfile_v3.xsd
