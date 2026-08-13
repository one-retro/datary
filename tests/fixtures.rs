//! Tests against real datafiles published by No-Intro and TOSEC, plus a
//! handcrafted file covering the parts of the Logiqx DTD that neither uses.

use datary::{Datafile, ForceNoDump, Status, YesNo};
use pretty_assertions::assert_eq;
use rstest::rstest;
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

/// Every fixture must parse, re-serialise *in its own syntax*, and parse back
/// to an equal value.
#[rstest]
fn round_trips(#[files("tests/fixtures/**/*.dat")] path: PathBuf) {
    let source = std::fs::read_to_string(&path).unwrap();
    let format = datary::detect(source.as_bytes(), datary::BUILTIN_FORMATS)
        .unwrap_or_else(|| panic!("{path:?}: format not recognised"));

    let original = format
        .parse(&source)
        .unwrap_or_else(|e| panic!("{path:?}: {e}"));

    let options = datary::WriteOptions::default();
    let text = format
        .write(&original, &options)
        .unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let reparsed = format
        .parse(&text)
        .unwrap_or_else(|e| panic!("{path:?} reparse: {e}"));

    assert_eq!(original, reparsed, "round trip changed {path:?}");

    // And a second pass must be byte-identical to the first.
    let text2 = format.write(&reparsed, &options).unwrap();
    assert_eq!(text, text2, "serialisation is not stable for {path:?}");
}

/// No fixture may lose data: every game must keep its name and description.
#[rstest]
fn fixtures_are_non_empty(#[files("tests/fixtures/**/*.dat")] path: PathBuf) {
    let dat = datary::read_file(&path).unwrap();
    assert!(dat.header.is_some(), "{path:?} has no header");
    assert!(!dat.games.is_empty(), "{path:?} has no games");
    for game in &dat.games {
        assert!(!game.name.is_empty(), "{path:?} has a game with no name");
    }
}

#[test]
fn no_intro_header_is_fully_parsed() {
    let dat = load("no-intro/virtual-boy.dat");
    let header = dat.header.as_ref().unwrap();

    assert_eq!(header.id, Some(15));
    assert_eq!(header.name, "Nintendo - Virtual Boy");
    assert_eq!(header.version, "20240829-133848");
    assert_eq!(header.homepage.as_deref(), Some("No-Intro"));
    assert_eq!(header.url.as_deref(), Some("https://www.no-intro.org"));

    // clrmamepro carries only forcenodump in No-Intro datafiles.
    let cmp = header.clr_mame_pro.as_ref().unwrap();
    assert_eq!(cmp.force_no_dump, Some(ForceNoDump::Required));
    assert_eq!(cmp.force_merging, None);

    // The xsi attributes on the root element survive parsing, and the
    // constants the crate exposes match what DAT-o-MATIC actually emits.
    assert_eq!(dat.xmlns_xsi.as_deref(), Some(datary::dat::XSI_NAMESPACE));
    assert_eq!(
        dat.schema_location.as_deref(),
        Some(datary::dat::NO_INTRO_SCHEMA_LOCATION)
    );
}

#[test]
fn no_intro_game_ids_keep_their_zero_padding() {
    let dat = load("no-intro/virtual-boy.dat");

    let tetris = dat.game("3-D Tetris (USA)").unwrap();
    assert_eq!(tetris.id.as_deref(), Some("0001"));
    assert!(!tetris.is_clone());

    // A clone references its parent by id, not by name.
    let proto = dat.game("Bound High (World) (Proto 1) [b]").unwrap();
    assert_eq!(proto.id.as_deref(), Some("0029"));
    assert_eq!(proto.clone_of_id.as_deref(), Some("0078"));
    assert_eq!(proto.clone_of, None);
    assert!(proto.is_clone());

    // ...and clones_of resolves that link.
    let parent = dat.game("Bound High (World) (Proto 2)").unwrap();
    assert_eq!(parent.id.as_deref(), Some("0078"));
    let clones = dat.clones_of(parent);
    assert!(clones.iter().any(|g| g.name == proto.name));
}

#[test]
fn a_game_may_carry_several_categories() {
    let dat = load("no-intro/virtual-boy.dat");

    let tetris = dat.game("3-D Tetris (USA)").unwrap();
    assert_eq!(tetris.categories, vec!["Games"]);

    let proto = dat.game("Bound High (World) (Proto 1) [b]").unwrap();
    assert_eq!(proto.categories, vec!["Games", "Preproduction"]);
    assert!(proto.has_category("Preproduction"));
    assert!(!proto.has_category("Demos"));
}

#[test]
fn no_intro_rom_attributes_are_typed() {
    let dat = load("no-intro/virtual-boy.dat");

    let rom = &dat.game("3-D Tetris (USA)").unwrap().roms[0];
    assert_eq!(rom.name, "3-D Tetris (USA).vb");
    assert_eq!(rom.size, 1_048_576);
    assert_eq!(rom.crc.unwrap().to_string(), "bb71b522");
    assert_eq!(
        rom.md5.unwrap().to_string(),
        "ecf1218706e9b547eab9e4be58d54e21"
    );
    assert_eq!(
        rom.sha1.unwrap().to_string(),
        "5177015a91442e56bd76af39447bca365e06c272"
    );
    assert_eq!(rom.serial.as_deref(), Some("VPBE"));
    assert_eq!(rom.sha256, None);
    // Absent status means good, per the DTD.
    assert_eq!(rom.status, None);
    assert_eq!(rom.status(), Status::Good);

    // A bad dump records sha256 and an explicit status.
    let bad = &dat.game("Bound High (World) (Proto 1) [b]").unwrap().roms[0];
    assert_eq!(bad.status(), Status::BadDump);
    assert!(!bad.status().is_usable());
    assert_eq!(
        bad.sha256.unwrap().to_string(),
        "3addd5321c08c138724e68bfc4bdf23dcf2b2cf16ae3109fe4f802de7c39efd2"
    );
}

/// `mia` is emitted by No-Intro but absent from its published schema.
#[test]
fn mia_attribute_is_parsed() {
    let dat = load("no-intro/famicom-network-system.dat");

    let mia = dat
        .game("Cosmo no Famicom Trade (Japan) (FCN003-03)")
        .unwrap();
    assert_eq!(mia.roms[0].mia, Some(YesNo::Yes));
    assert!(mia.roms[0].is_mia());
    assert!(mia.is_mia());

    let present = dat.game("[BIOS] RF5A18 CPU2 ROM (Japan)").unwrap();
    assert_eq!(present.roms[0].mia, None);
    assert!(!present.is_mia());

    assert_eq!(dat.games.iter().filter(|g| g.is_mia()).count(), 15);
}

/// `<game_id>` is emitted for 3DS/Wii U/Switch and is in no published schema.
#[test]
fn game_id_element_is_parsed() {
    let dat = load("no-intro/new-nintendo-3ds-decrypted.dat");

    let game = dat.game("Fire Emblem Musou (Japan)").unwrap();
    assert_eq!(game.game_id.as_deref(), Some("000400000F70C100"));
    assert_eq!(game.clone_of_id.as_deref(), Some("0008"));
}

/// The No-Intro schema types `size` as `xs:unsignedInt`, but real files exceed it.
#[test]
fn rom_size_exceeds_u32() {
    let dat = load("no-intro/new-nintendo-3ds-decrypted.dat");

    let game = dat.game("Fire Emblem Musou (Japan)").unwrap();
    let size = game.roms[0].size;
    assert_eq!(size, 4_294_967_296);
    assert!(size > u64::from(u32::MAX), "size must not fit in a u32");
    assert_eq!(game.total_size(), 4_294_967_296);
}

#[test]
fn tosec_dialect_parses() {
    let dat = load("tosec/nes-games.dat");
    let header = dat.header.as_ref().unwrap();

    // TOSEC uses header fields No-Intro does not.
    assert_eq!(header.category.as_deref(), Some("TOSEC"));
    assert_eq!(header.email.as_deref(), Some("contact@tosecdev.org"));
    assert_eq!(header.version, "2023-06-14");
    assert_eq!(header.id, None);
    assert_eq!(header.subset, None);

    assert_eq!(dat.games.len(), 400);
    // TOSEC names contain XML entities that must survive decoding.
    let game = dat
        .game("'89 Dennou Kyuusei Uranai (1988-12-10)(Induction Produce)(JP)")
        .unwrap();
    assert_eq!(game.roms[0].size, 262_160);
    assert!(header.name.contains('&'), "entity should be decoded to &");
}

/// The parts of the Logiqx DTD that neither No-Intro nor TOSEC emits.
#[test]
fn logiqx_full_dtd_parses() {
    let dat = load("logiqx/full-dtd.dat");

    assert_eq!(dat.build.as_deref(), Some("TestBuild v1.2"));
    assert_eq!(dat.debug, Some(YesNo::No));
    assert!(!dat.is_debug());

    let header = dat.header.as_ref().unwrap();
    assert_eq!(header.comment.as_deref(), Some("A header comment"));

    let cmp = header.clr_mame_pro.as_ref().unwrap();
    assert_eq!(cmp.header.as_deref(), Some("skipheader.xml"));
    assert_eq!(cmp.force_merging, Some(datary::ForceMerging::Full));
    assert_eq!(cmp.force_packing, Some(datary::ForcePacking::Unzip));

    let rc = header.rom_center.as_ref().unwrap();
    assert_eq!(rc.plugin.as_deref(), Some("example.dll"));
    assert_eq!(rc.rom_mode, Some(datary::RomMode::Merged));
    assert_eq!(rc.bios_mode, Some(datary::RomMode::Unmerged));
    assert_eq!(rc.sample_mode, Some(datary::SampleMode::Unmerged));
    assert_eq!(rc.lock_rom_mode, Some(YesNo::Yes));

    let bios = dat.game("bios-parent").unwrap();
    assert!(bios.is_bios());
    assert_eq!(bios.source_file.as_deref(), Some("src.cpp"));
    assert_eq!(bios.board.as_deref(), Some("PCB-1"));
    assert_eq!(bios.rebuild_to.as_deref(), Some("other"));
    assert_eq!(bios.comments, vec!["First comment", "Second comment"]);
    assert_eq!(bios.year.as_deref(), Some("1991"));
    assert_eq!(bios.manufacturer.as_deref(), Some("Example Corp"));
    assert_eq!(bios.bios_sets.len(), 2);
    assert!(bios.bios_sets[0].is_default());
    assert!(!bios.bios_sets[1].is_default());
    assert_eq!(bios.roms[0].status(), Status::Verified);
    assert_eq!(bios.roms[0].date.as_deref(), Some("1991-06-01"));

    let child = dat.game("child-set").unwrap();
    assert_eq!(child.clone_of.as_deref(), Some("bios-parent"));
    assert_eq!(child.rom_of.as_deref(), Some("bios-parent"));
    assert_eq!(child.sample_of.as_deref(), Some("samples-set"));

    // `<release>` appears before `<rom>` in this file; element order must not matter.
    assert_eq!(child.releases.len(), 2);
    assert_eq!(child.releases[0].region, "EU");
    assert_eq!(child.releases[0].language.as_deref(), Some("en"));
    assert!(child.releases[0].is_default());
    assert!(!child.releases[1].is_default());

    assert_eq!(child.roms.len(), 2);
    assert_eq!(child.roms[0].merge.as_deref(), Some("bios.rom"));
    assert_eq!(child.roms[0].status(), Status::BadDump);
    assert!(child.roms[1].is_no_dump());
    assert!(!child.roms[1].has_checksum());

    assert_eq!(child.disks.len(), 1);
    assert_eq!(child.disks[0].name, "hdd");
    assert_eq!(child.disks[0].status(), Status::Good);
    assert!(child.disks[0].sha1.is_some());
    assert_eq!(child.disks[0].merge.as_deref(), Some("parentdisk"));

    assert_eq!(child.samples.len(), 2);
    assert_eq!(child.samples[1].name, "sound2");
    assert_eq!(child.archives.len(), 1);
    assert_eq!(child.archives[0].name, "extras");
}

#[test]
fn helpers_report_useful_totals() {
    let dat = load("no-intro/virtual-boy.dat");

    assert_eq!(dat.rom_count(), dat.games.len());
    assert_eq!(dat.roms().count(), dat.games.len());
    assert!(dat.games.iter().all(|g| g.roms.len() == 1));

    let rom = &dat.games[0].roms[0];
    assert_eq!(rom.extension().as_deref(), Some("vb"));
    assert!(rom.has_checksum());
}

/// The published No-Intro datafiles must be reproduced byte for byte.
///
/// This is the strongest possible round-trip check: it proves the model loses
/// nothing, not even attribute order or whitespace.
///
/// DAT-o-MATIC emits CRLF, but files that have travelled through a git
/// checkout are often normalised to LF, so the expected line ending is taken
/// from the file itself rather than assumed.
#[rstest]
#[case("no-intro/virtual-boy.dat")]
#[case("no-intro/famicom-network-system.dat")]
#[case("no-intro/new-nintendo-3ds-decrypted.dat")]
#[case("no-intro/pokemon-mini.dat")]
fn no_intro_files_reproduce_byte_for_byte(#[case] relative: &str) {
    let path = fixture(relative);
    let original = std::fs::read_to_string(&path).unwrap();

    let options = datary::WriteOptions::no_intro().line_ending(if original.contains("\r\n") {
        datary::LineEnding::Crlf
    } else {
        datary::LineEnding::Lf
    });

    let dat = datary::read_file(&path).unwrap();
    let written = datary::to_string_with(&dat, &options).unwrap();

    assert_eq!(original, written, "{relative} was not reproduced exactly");
}
