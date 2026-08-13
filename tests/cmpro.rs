//! Tests for the native ClrMamePro syntax, and for format detection.

#![cfg(feature = "cmpro")]

use datary::format::{detect, ClrMamePro, DatFormat, Xml, BUILTIN_FORMATS};
use datary::{Datafile, Error, Status, WriteOptions};
use pretty_assertions::assert_eq;
use std::path::{Path, PathBuf};

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn load(relative: &str) -> Datafile {
    datary::read_file(fixture(relative))
        .unwrap_or_else(|e| panic!("failed to read {relative}: {e}"))
}

#[test]
fn a_real_ckmame_datafile_parses() {
    let dat = load("cmpro/ckmame-mame.dat");

    let header = dat.header.as_ref().unwrap();
    assert_eq!(header.name, "ckmame test db");
    assert_eq!(header.version, "1");

    assert_eq!(dat.games.len(), 20);

    let game = dat.game("1-4").unwrap();
    assert_eq!(game.description, "one four byte file");
    assert_eq!(game.manufacturer.as_deref(), Some("synth"));
    assert_eq!(game.year.as_deref(), Some("1991"));

    let rom = &game.roms[0];
    assert_eq!(rom.name, "04.rom");
    assert_eq!(rom.size, 4);
    // ckmame spells the attribute `crc32`; ClrMamePro spells it `crc`.
    assert_eq!(rom.crc.unwrap().to_string(), "d87f7e0c");
    assert_eq!(
        rom.sha1.unwrap().to_string(),
        "a94a8fe5ccb19ba61c4c0873d391e987982fbbd3"
    );
}

#[test]
fn begin_and_end_markers_are_ignored() {
    // ckmame-disk.dat is wrapped in BEGIN/END.
    let dat = load("cmpro/ckmame-disk.dat");
    assert_eq!(dat.games.len(), 1);
    assert_eq!(dat.header.as_ref().unwrap().name, "game-with-disk");
}

#[test]
fn nested_disk_blocks_parse() {
    let dat = load("cmpro/ckmame-disk.dat");
    let game = dat.game("disk").unwrap();

    assert_eq!(game.roms.len(), 1);
    assert_eq!(game.disks.len(), 1);

    let disk = &game.disks[0];
    assert_eq!(disk.name, "108-5");
    assert_eq!(
        disk.md5.unwrap().to_string(),
        "bf5c9c39eb49bcf5a55a06dbb4deccb3"
    );
    assert_eq!(
        disk.sha1.unwrap().to_string(),
        "7570a907e20a51cbf6193ec6779b82d1967bb609"
    );
}

/// CMPro spells the dump status `flags`, not `status`.
#[test]
fn flags_map_onto_status() {
    let dat = datary::cmpro::from_str(
        r"game (
            name g
            rom ( name a.rom size 1 crc 00000001 flags baddump )
            rom ( name b.rom size 0 flags nodump )
            rom ( name c.rom size 1 crc 00000002 flags verified )
            rom ( name d.rom size 1 crc 00000003 )
        )",
    )
    .unwrap();

    let roms = &dat.games[0].roms;
    assert_eq!(roms[0].status(), Status::BadDump);
    assert_eq!(roms[1].status(), Status::NoDump);
    assert_eq!(roms[2].status(), Status::Verified);
    // Absent means good, and stays absent in the model.
    assert_eq!(roms[3].status, None);
    assert_eq!(roms[3].status(), Status::Good);
}

/// MAME's `-listinfo` output uses the same grammar with a different header.
#[test]
fn mame_listinfo_header_is_understood() {
    let dat = datary::cmpro::from_str(
        r#"emulator (
            name "MAME 0.999"
            version 0.999
        )
        game ( name g description "G" )"#,
    )
    .unwrap();

    assert_eq!(dat.header.as_ref().unwrap().name, "MAME 0.999");
    assert_eq!(dat.games.len(), 1);
}

#[test]
fn bare_and_quoted_values_are_equivalent() {
    let bare = datary::cmpro::from_str("game ( name g description d )").unwrap();
    let quoted = datary::cmpro::from_str(r#"game ( name "g" description "d" )"#).unwrap();
    assert_eq!(bare, quoted);
}

#[test]
fn round_trips_through_its_own_syntax() {
    let original = load("cmpro/ckmame-mame.dat");

    let text = datary::cmpro::to_string(&original);
    let reparsed = datary::cmpro::from_str(&text).unwrap();

    assert_eq!(original, reparsed);

    // And a second pass is byte-stable.
    assert_eq!(text, datary::cmpro::to_string(&reparsed));
}

/// The whole point of one shared model: a file read in one syntax can be
/// written in the other.
#[test]
fn converts_between_xml_and_clrmamepro() {
    let from_xml = load("no-intro/virtual-boy.dat");

    let cmpro_text = ClrMamePro
        .write(&from_xml, &WriteOptions::clrmamepro())
        .unwrap();
    let via_cmpro = ClrMamePro.parse(&cmpro_text).unwrap();

    // CMPro cannot express the No-Intro-only fields, so compare what it can.
    assert_eq!(via_cmpro.games.len(), from_xml.games.len());
    for (a, b) in via_cmpro.games.iter().zip(&from_xml.games) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.roms.len(), b.roms.len());
        assert_eq!(a.roms[0].size, b.roms[0].size);
        assert_eq!(a.roms[0].crc, b.roms[0].crc);
        assert_eq!(a.roms[0].sha1, b.roms[0].sha1);
    }

    // Going back to XML preserves what survived the trip.
    let xml = Xml.write(&via_cmpro, &WriteOptions::default()).unwrap();
    assert_eq!(Xml.parse(&xml).unwrap(), via_cmpro);
}

#[test]
fn detection_picks_the_right_syntax() {
    // read_file sniffs content, because both syntaxes use a .dat extension.
    let xml = load("no-intro/virtual-boy.dat");
    let cmpro = load("cmpro/ckmame-mame.dat");
    assert_eq!(xml.header.as_ref().unwrap().name, "Nintendo - Virtual Boy");
    assert_eq!(cmpro.header.as_ref().unwrap().name, "ckmame test db");

    assert_eq!(
        detect(b"<datafile/>", BUILTIN_FORMATS).unwrap().name(),
        "XML"
    );
    assert_eq!(
        detect(b"clrmamepro ( name x )", BUILTIN_FORMATS)
            .unwrap()
            .name(),
        "ClrMamePro"
    );
}

#[test]
fn forcing_the_wrong_syntax_fails() {
    let source = std::fs::read_to_string(fixture("no-intro/virtual-boy.dat")).unwrap();
    // XML forced through the CMPro parser is a syntax error, not silence.
    assert!(ClrMamePro.parse(&source).is_err());
}

#[test]
fn empty_input_cannot_be_detected() {
    assert!(matches!(
        datary::from_str("   \n\t "),
        Err(Error::UnknownFormat)
    ));
}

#[test]
fn syntax_errors_carry_a_line_and_column() {
    let err = datary::cmpro::from_str("game (\n\tname g\n\tbad \"unterminated\n)").unwrap_err();
    let Error::Cmpro(e) = err else {
        panic!("expected a Cmpro error, got {err:?}");
    };

    let position = e.position.expect("a syntax error carries a position");
    assert_eq!(position.line, 3, "should point at the unterminated quote");
    assert!(e.message.contains("closing"), "{}", e.message);
    // The Display impl includes the position.
    assert!(e.to_string().contains("line 3"), "{e}");
}

#[test]
fn values_needing_quotes_get_them() {
    let dat = Datafile {
        games: vec![datary::Game {
            name: "Has Spaces (USA)".into(),
            description: "plain".into(),
            roms: vec![datary::Rom {
                name: "simple.rom".into(),
                size: 1,
                ..datary::Rom::default()
            }],
            ..datary::Game::default()
        }],
        ..Datafile::default()
    };

    let text = datary::cmpro::to_string(&dat);
    assert!(text.contains(r#"name "Has Spaces (USA)""#), "{text}");
    // Values that need no quoting are left bare, as producers do.
    assert!(text.contains("description plain"), "{text}");
    assert!(text.contains("name simple.rom"), "{text}");

    assert_eq!(datary::cmpro::from_str(&text).unwrap(), dat);
}

/// A downstream crate can add a format and use the same helpers.
#[test]
fn a_third_party_format_plugs_in() {
    struct Csv;

    impl DatFormat for Csv {
        fn name(&self) -> &'static str {
            "CSV"
        }

        fn detect(&self, bytes: &[u8]) -> bool {
            bytes.starts_with(b"name,size")
        }

        fn parse(&self, source: &str) -> datary::Result<Datafile> {
            let games = source
                .lines()
                .skip(1)
                .filter(|l| !l.is_empty())
                .map(|line| {
                    let (name, size) = line.split_once(',').unwrap_or((line, "0"));
                    datary::Game {
                        name: name.to_string(),
                        description: name.to_string(),
                        roms: vec![datary::Rom {
                            name: name.to_string(),
                            size: size.parse().unwrap_or(0),
                            ..datary::Rom::default()
                        }],
                        ..datary::Game::default()
                    }
                })
                .collect();
            Ok(Datafile {
                games,
                ..Datafile::default()
            })
        }

        fn write(&self, dat: &Datafile, _: &WriteOptions) -> datary::Result<String> {
            let mut out = String::from("name,size\n");
            for game in &dat.games {
                out.push_str(&format!("{},{}\n", game.name, game.total_size()));
            }
            Ok(out)
        }
    }

    let source = "name,size\nGame A,1024\nGame B,2048\n";

    // Explicit use.
    let dat = datary::from_str_as(source, &Csv).unwrap();
    assert_eq!(dat.games.len(), 2);
    assert_eq!(dat.games[0].name, "Game A");
    assert_eq!(dat.games[1].roms[0].size, 2048);

    // Detection over a custom list, alongside the built-ins.
    let formats: &[&dyn DatFormat] = &[&Csv, &Xml, &ClrMamePro];
    assert_eq!(detect(source.as_bytes(), formats).unwrap().name(), "CSV");
    assert_eq!(
        detect(b"<datafile/>", formats).unwrap().name(),
        "XML",
        "a custom format must not shadow the built-ins"
    );

    // And writing goes through the same helper.
    let text = datary::to_string_as(&dat, &Csv, &WriteOptions::default()).unwrap();
    assert_eq!(text, source);
}

/// A malformed checksum must be an error, never silently dropped.
///
/// Dropping it would leave the entry with no checksum, and `Rom::verify` does
/// not check what an entry does not record — so a typo would produce a ROM that
/// verifies against any file of the right size.
#[test]
fn a_malformed_checksum_is_rejected_not_ignored() {
    for (label, source) in [
        ("sha1 too short", "game ( rom ( name a size 1 sha1 abcd ) )"),
        (
            "md5 too long",
            "game ( rom ( name a size 1 md5 0123456789abcdef0123456789abcdef00 ) )",
        ),
        ("crc not hex", "game ( rom ( name a size 1 crc zzzzzzzz ) )"),
        (
            "crc32 alias not hex",
            "game ( rom ( name a size 1 crc32 nothex12 ) )",
        ),
        (
            "sha256 too short",
            "game ( rom ( name a size 1 sha256 abcd ) )",
        ),
        (
            "disk sha1 not hex",
            "game ( disk ( name d sha1 qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq ) )",
        ),
    ] {
        let result = datary::cmpro::from_str(source);
        assert!(result.is_err(), "{label} was accepted: {source}");

        // The message must name the offending key and value.
        let message = result.unwrap_err().to_string();
        assert!(
            message.contains("invalid"),
            "{label}: unhelpful message {message:?}"
        );
    }
}

/// Both syntaxes must reject the same bad data. An asymmetry here means one
/// front-end silently accepts what the other rejects.
#[test]
fn both_syntaxes_reject_a_bad_checksum() {
    let cmpro = datary::cmpro::from_str("game ( rom ( name a size 1 sha1 abcd ) )");
    let xml = datary::from_str(
        r#"<datafile><game name="g"><description>d</description>
             <rom name="a" size="1" sha1="abcd"/>
           </game></datafile>"#,
    );

    assert!(cmpro.is_err(), "ClrMamePro accepted a 4-digit sha1");
    assert!(xml.is_err(), "XML accepted a 4-digit sha1");
}

/// A valid checksum in either dialect spelling still parses.
#[test]
fn valid_checksums_still_parse() {
    let dat = datary::cmpro::from_str(
        "game ( rom ( name a size 1 crc32 d87f7e0c sha1 a94a8fe5ccb19ba61c4c0873d391e987982fbbd3 ) )",
    )
    .unwrap();
    let rom = &dat.games[0].roms[0];
    assert_eq!(rom.crc.unwrap().to_string(), "d87f7e0c");
    assert!(rom.sha1.is_some());
}

/// Both syntax and value errors are located in the source.
#[test]
fn every_error_carries_a_position() {
    for (label, source) in [
        ("syntax", "game (\n"),
        ("value", "game ( rom ( name a size 1 sha1 abcd ) )"),
    ] {
        let Error::Cmpro(e) = datary::cmpro::from_str(source).unwrap_err() else {
            panic!("{label}: expected a Cmpro error");
        };
        let position = e
            .position
            .unwrap_or_else(|| panic!("{label} errors must be located"));
        assert!(position.offset <= source.len(), "{label}: offset in range");
        assert!(
            position.line >= 1 && position.column >= 1,
            "{label}: 1-based"
        );
        assert!(e.to_string().contains("line"), "{label}: {e}");
    }
}

/// ckmame and ClrMamePro sometimes prefix a checksum with `0x`.
///
/// Found in ckmame's own corpus (its `deadbeefish` set), not invented.
#[test]
fn hex_prefixed_checksums_are_accepted() {
    let dat = datary::cmpro::from_str(
        "game ( rom ( name a size 4 crc32 0xd87f7e0c sha1 0X0b0dcdf77237b4e5d920990b92d4b59ad264910f ) )",
    )
    .unwrap();

    let rom = &dat.games[0].roms[0];
    // The prefix is stripped, not stored.
    assert_eq!(rom.crc.unwrap().to_string(), "d87f7e0c");
    assert_eq!(
        rom.sha1.unwrap().to_string(),
        "0b0dcdf77237b4e5d920990b92d4b59ad264910f"
    );

    // A prefixed value equals its bare form.
    let bare = datary::cmpro::from_str("game ( rom ( name a size 4 crc32 d87f7e0c ) )").unwrap();
    assert_eq!(rom.crc, bare.games[0].roms[0].crc);

    // The prefix does not excuse a bad value.
    assert!(datary::cmpro::from_str("game ( rom ( name a size 1 crc 0xzzzz ) )").is_err());
}

/// Errors carry a byte offset as well as a line and column, so a caller can
/// hand the span straight to a diagnostic renderer.
#[test]
fn errors_carry_a_byte_offset() {
    // Syntax error.
    let source = "game (\n\tname \"oops\n)\n";
    let Error::Cmpro(e) = datary::cmpro::from_str(source).unwrap_err() else {
        panic!("expected a Cmpro error");
    };
    let p = e.position.expect("syntax errors are located");
    assert_eq!(p.line, 2);
    // The offset must index the source at the reported line.
    assert!(p.offset < source.len());
    assert_eq!(
        source[..p.offset].matches('\n').count() + 1,
        p.line,
        "offset and line must agree"
    );

    // Value error: located too, via the borrowed slice.
    let source = "game (\n\trom ( name a size 1 sha1 abcd )\n)";
    let Error::Cmpro(e) = datary::cmpro::from_str(source).unwrap_err() else {
        panic!("expected a Cmpro error");
    };
    let p = e.position.expect("value errors are located too");
    assert_eq!(p.line, 2, "the bad sha1 is on line 2");
    assert_eq!(
        &source[p.offset..p.offset + 4],
        "abcd",
        "the offset must point at the offending value"
    );
}
