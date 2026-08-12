//! Tests for the lookup tables and file verification.

#![cfg(all(feature = "index", feature = "verify"))]

use datary::hash::{Crc32, Sha1};
use datary::verify::{FileHashes, Mismatch};
use datary::{Datafile, Game, Rom};
use pretty_assertions::assert_eq;
use std::path::{Path, PathBuf};
use std::str::FromStr;

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

/// Builds a datafile whose ROM contents are known, so hashes can be computed.
fn datafile_of(entries: &[(&str, &str, &[u8])]) -> Datafile {
    Datafile {
        games: entries
            .iter()
            .map(|(game, rom, data)| {
                let h = FileHashes::of_bytes(data);
                Game {
                    name: (*game).to_string(),
                    description: (*game).to_string(),
                    roms: vec![Rom {
                        name: (*rom).to_string(),
                        size: h.size,
                        crc: Some(h.crc),
                        md5: Some(h.md5),
                        sha1: Some(h.sha1),
                        sha256: Some(h.sha256),
                        ..Rom::default()
                    }],
                    ..Game::default()
                }
            })
            .collect(),
        ..Datafile::default()
    }
}

#[test]
fn lookups_find_the_right_game() {
    let dat = datary::read_file(fixture("no-intro/virtual-boy.dat"))
        .unwrap()
        .indexed();

    let sha1 = Sha1::from_str("5177015a91442e56bd76af39447bca365e06c272").unwrap();
    assert_eq!(dat.game_by_sha1(&sha1).unwrap().name, "3-D Tetris (USA)");

    let crc = Crc32::from_str("bb71b522").unwrap();
    assert_eq!(dat.game_by_crc(crc).unwrap().name, "3-D Tetris (USA)");

    assert_eq!(
        dat.game_by_name("3-D Tetris (USA)").unwrap().id.as_deref(),
        Some("0001")
    );
    assert_eq!(dat.game_by_id("0001").unwrap().name, "3-D Tetris (USA)");

    let (game, rom) = dat.by_rom_name("3-D Tetris (USA).vb").unwrap();
    assert_eq!(game.name, "3-D Tetris (USA)");
    assert_eq!(rom.serial.as_deref(), Some("VPBE"));
}

#[test]
fn missing_keys_return_nothing() {
    let dat = datary::read_file(fixture("no-intro/virtual-boy.dat"))
        .unwrap()
        .indexed();

    let absent = Sha1::from_str("0000000000000000000000000000000000000000").unwrap();
    assert!(dat.game_by_sha1(&absent).is_none());
    assert!(dat.index().by_sha1(&absent).is_empty());
    assert!(dat.game_by_crc(Crc32(0xdead_beef)).is_none());
    assert!(dat.game_by_name("Nonexistent").is_none());
    assert!(dat.by_rom_name("nope.vb").is_none());
    assert_eq!(dat.by_rom_name_prefix("zzzz").count(), 0);
}

#[test]
fn prefix_search_is_ordered_and_bounded() {
    let dat = datary::read_file(fixture("tosec/nes-games.dat"))
        .unwrap()
        .indexed();

    let names: Vec<_> = dat
        .by_rom_name_prefix("10 Yard Fight")
        .map(|(_, rom)| rom.name.clone())
        .collect();

    assert!(!names.is_empty());
    assert!(names.iter().all(|n| n.starts_with("10 Yard Fight")));
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "prefix results must be in name order");
}

#[test]
fn several_games_can_share_a_checksum() {
    // Two distinct games holding an identical file.
    let dat = datafile_of(&[
        ("Game A", "a.rom", b"identical"),
        ("Game B", "b.rom", b"identical"),
        ("Game C", "c.rom", b"different"),
    ])
    .indexed();

    let h = FileHashes::of_bytes(b"identical");
    let games: Vec<_> = dat
        .games_by_sha1(&h.sha1)
        .map(|g| g.name.as_str())
        .collect();
    assert_eq!(games, vec!["Game A", "Game B"]);
    assert_eq!(dat.index().by_sha1(&h.sha1).len(), 2);

    // The single-result accessor returns the first.
    assert_eq!(dat.game_by_sha1(&h.sha1).unwrap().name, "Game A");
}

#[test]
fn a_game_with_a_duplicated_rom_is_yielded_once() {
    let h = FileHashes::of_bytes(b"dup");
    let rom = Rom {
        name: "dup.rom".into(),
        size: h.size,
        sha1: Some(h.sha1),
        ..Rom::default()
    };
    let dat = Datafile {
        games: vec![Game {
            name: "Twice".into(),
            roms: vec![rom.clone(), rom],
            ..Game::default()
        }],
        ..Datafile::default()
    }
    .indexed();

    assert_eq!(dat.index().by_sha1(&h.sha1).len(), 2, "both roms indexed");
    assert_eq!(dat.games_by_sha1(&h.sha1).count(), 1, "one game yielded");
}

#[test]
fn find_matches_a_hashed_file() {
    let dat = datafile_of(&[("Game A", "a.rom", b"contents of a")]).indexed();

    let hashes = FileHashes::of_bytes(b"contents of a");
    let (game, rom) = dat.find(&hashes).unwrap();
    assert_eq!(game.name, "Game A");
    assert_eq!(rom.name, "a.rom");

    assert!(dat.find(&FileHashes::of_bytes(b"something else")).is_none());
}

#[test]
fn find_rejects_a_crc_collision_that_fails_a_stronger_hash() {
    // An entry whose CRC matches the file but whose SHA-1 does not.
    let real = FileHashes::of_bytes(b"the real file");
    let other = FileHashes::of_bytes(b"an impostor!!");

    let dat = Datafile {
        games: vec![Game {
            name: "Bogus".into(),
            roms: vec![Rom {
                name: "bogus.rom".into(),
                size: other.size,
                crc: Some(real.crc), // pretend collision
                sha1: Some(other.sha1),
                ..Rom::default()
            }],
            ..Game::default()
        }],
        ..Datafile::default()
    }
    .indexed();

    // Looked up by CRC, but rejected because the SHA-1 disagrees.
    assert!(!dat.index().by_crc(real.crc).is_empty());
    assert!(dat.find(&real).is_none());
}

#[test]
fn verifying_a_real_file_reports_each_difference() {
    let dat = datafile_of(&[("Game A", "a.rom", b"hello world")]);
    let rom = &dat.games[0].roms[0];

    assert!(rom.matches(&FileHashes::of_bytes(b"hello world")));

    let problems = rom
        .verify(&FileHashes::of_bytes(b"HELLO WORLD"))
        .unwrap_err();
    assert_eq!(problems.len(), 4, "same size, four checksums differ");
    assert!(problems.iter().any(|p| matches!(p, Mismatch::Sha1 { .. })));
    // The message names both sides.
    assert!(problems[0].to_string().contains("expected"));
}

#[test]
fn hashing_a_file_on_disk_matches_hashing_its_bytes() {
    let path = fixture("no-intro/virtual-boy.dat");
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        FileHashes::of_file(&path).unwrap(),
        FileHashes::of_bytes(&bytes)
    );
}

#[test]
fn modifying_rebuilds_the_index() {
    let mut dat = datafile_of(&[("Game A", "a.rom", b"a")]).indexed();
    assert!(dat.game_by_name("Game B").is_none());

    dat.modify(|d| {
        d.games.push(Game {
            name: "Game B".into(),
            ..Game::default()
        });
    });

    assert!(dat.game_by_name("Game B").is_some());
    assert_eq!(dat.games.len(), 2);
}

#[test]
fn indexed_datafile_derefs_and_unwraps() {
    let original = datafile_of(&[("Game A", "a.rom", b"a")]);
    let indexed = original.clone().indexed();

    // Deref exposes the datafile's own API.
    assert_eq!(indexed.games.len(), 1);
    assert_eq!(indexed.rom_count(), 1);
    assert!(indexed.game("Game A").is_some());

    assert_eq!(indexed.into_inner(), original);
}

#[test]
fn index_reports_its_size() {
    let dat = datafile_of(&[("A", "a.rom", b"a"), ("B", "b.rom", b"b")]);
    let index = datary::Index::build(&dat);
    assert_eq!(index.len(), 2);
    assert!(!index.is_empty());

    assert!(datary::Index::build(&Datafile::default()).is_empty());
}
