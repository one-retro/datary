//! Tests driving the `dat` binary end to end.
//!
//! Each runs in the crate root with relative fixture paths, so the paths that
//! appear in the output are the same on every platform.

use datary::format::{ClrMamePro, DatFormat};
use datary::WriteOptions;
use pretty_assertions::assert_eq;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const VIRTUAL_BOY: &str = "tests/fixtures/no-intro/virtual-boy.dat";
const BADDUMP: &str = "tests/fixtures/cmpro/ckmame-baddump.dat";
const DISK: &str = "tests/fixtures/cmpro/ckmame-disk.dat";
const POKEMON_MINI: &str = "tests/fixtures/no-intro/pokemon-mini.dat";

/// A ClrMamePro datafile in ISO-8859-1: `\xe9` is `é` and `\xdf` is `ß`, and
/// neither byte is valid UTF-8 on its own.
const LATIN1: &[u8] =
    b"game (\n\tname \"Pok\xe9mon Stra\xdfe\"\n\tdescription \"Pok\xe9mon Stra\xdfe\"\n)\n";

/// A path relative to the crate root, made absolute for reading in-process.
fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// Runs `dat` with `args`, feeding it `stdin`.
fn run(args: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dat"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("dat runs");
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

fn dat(args: &[&str]) -> Output {
    run(args, b"")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn text_is_the_default_and_shows_the_header_then_each_game() {
    let output = dat(&[BADDUMP]);
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        "\
header:
  name: ckmame test db
  version: 1

baddump:
  description: bad dump
  year: 1994
  manufacturer: synth
  roms:
    - name: bad.rom
      size: 3
      crc: 148c7b71
      status: baddump
"
    );
}

#[test]
fn text_is_never_styled_when_not_a_terminal() {
    let output = dat(&[VIRTUAL_BOY]);
    assert!(!output.stdout.contains(&0x1b));
}

#[test]
fn a_title_search_matches_words_in_order() {
    let output = dat(&["--title", "BOUND high", VIRTUAL_BOY]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(
        text.starts_with("Bound High (World) (Proto 1) [b]:\n"),
        "{text}"
    );
    assert!(text.contains("\nBound High (World) (Proto 2):\n"));
    assert!(!text.contains("header:"), "a search shows only the matches");
}

#[test]
fn a_failed_title_search_suggests_the_closest_title() {
    let output = dat(&["--title", "bond hihg", VIRTUAL_BOY]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        "no game matches title \"bond hihg\"; did you mean \"Bound High (World) (Proto 2)\"?\n"
    );
}

#[test]
fn a_failed_title_search_suggests_nothing_when_nothing_is_close() {
    let output = dat(&["--title", "xyzzy", VIRTUAL_BOY]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr(&output), "no game matches title \"xyzzy\"\n");
}

#[test]
fn a_hash_search_finds_disks_and_ignores_case() {
    let output = dat(&[
        "--sha1",
        "7570A907E20A51CBF6193EC6779B82D1967BB609",
        BADDUMP,
        DISK,
    ]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(
        text.starts_with("==> tests/fixtures/cmpro/ckmame-disk.dat <==\ndisk:\n"),
        "{text}"
    );
    assert!(
        !text.contains("ckmame-baddump"),
        "inputs without a match are left out"
    );

    let output = dat(&["--md5", "bf5c9c39eb49bcf5a55a06dbb4deccb3", DISK]);
    assert!(stdout(&output).starts_with("disk:\n"));
}

#[test]
fn a_sha256_search_finds_roms_and_ignores_case() {
    let output = dat(&[
        "--sha256",
        "3ADDD5321C08C138724E68BFC4BDF23DCF2B2CF16AE3109FE4F802DE7C39EFD2",
        VIRTUAL_BOY,
    ]);
    assert!(output.status.success());
    assert!(stdout(&output).starts_with("Bound High (World) (Proto 1) [b]:\n"));
    assert!(!stdout(&output).contains("(Proto 2)"));
}

#[test]
fn a_title_search_ignores_accents() {
    let output = dat(&["--title", "POKÉMON party mini (usa)", POKEMON_MINI]);
    assert!(output.status.success());
    assert!(stdout(&output).starts_with("Pokemon Party Mini (USA):\n"));
}

#[test]
fn a_failed_hash_search_exits_with_one() {
    let output = dat(&["--md5", "00000000000000000000000000000000", DISK]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "no game matches md5 00000000000000000000000000000000\n"
    );
}

#[test]
fn malformed_and_conflicting_queries_are_usage_errors() {
    let output = dat(&["--sha1", "xyz", DISK]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("expected 40 hex digits, got 3"));

    let output = dat(&[
        "--md5",
        "bf5c9c39eb49bcf5a55a06dbb4deccb3",
        "--title",
        "x",
        DISK,
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("cannot be used with"));

    let output = dat(&["--sha1", &"0".repeat(40), "--sha256", &"0".repeat(64), DISK]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("cannot be used with"));

    let output = dat(&["--title", "  ", DISK]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn format_names_ignore_case_and_accept_aliases() {
    let expected = stdout(&dat(&["--format", "dat", DISK]));
    for format in ["DAT", "Dat", "xml", "XML"] {
        assert_eq!(
            stdout(&dat(&["--format", format, DISK])),
            expected,
            "--format {format}"
        );
    }
    let expected = stdout(&dat(&["--format", "mame", DISK]));
    for format in ["MAME", "cmpro", "ClrMamePro"] {
        assert_eq!(
            stdout(&dat(&["-f", format, DISK])),
            expected,
            "--format {format}"
        );
    }
}

#[test]
fn dat_output_is_the_library_xml_writer() {
    let output = dat(&["--format", "dat", VIRTUAL_BOY]);
    let dat = datary::read_file(fixture(VIRTUAL_BOY)).unwrap();
    assert_eq!(stdout(&output), datary::to_string(&dat).unwrap());
}

#[test]
fn mame_output_is_the_library_clrmamepro_writer() {
    let output = dat(&["--format", "mame", VIRTUAL_BOY]);
    let dat = datary::read_file(fixture(VIRTUAL_BOY)).unwrap();
    let expected = ClrMamePro.write(&dat, &WriteOptions::clrmamepro()).unwrap();
    assert_eq!(stdout(&output), expected);
}

#[test]
fn standard_input_is_read_when_no_path_is_given() {
    let mame = dat(&["--format", "mame", VIRTUAL_BOY]).stdout;
    // Reading ClrMamePro back in and writing it out again changes nothing.
    assert_eq!(run(&["--format", "mame"], &mame).stdout, mame);
    assert_eq!(run(&["--format", "mame", "-"], &mame).stdout, mame);
}

#[test]
fn a_search_writes_a_datafile_of_just_the_matches() {
    let output = dat(&["--format", "dat", "--title", "tetris", VIRTUAL_BOY]);
    let dat = datary::from_bytes(&output.stdout).unwrap();
    let names: Vec<&str> = dat.games.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["3-D Tetris (USA)", "V-Tetris (Japan) (En)"]);
    assert_eq!(
        dat.header.unwrap().name,
        "Nintendo - Virtual Boy",
        "one input keeps its header"
    );
}

#[test]
fn several_inputs_combine_into_one_datafile_without_a_header() {
    let output = dat(&["--format", "dat", BADDUMP, DISK]);
    let dat = datary::from_bytes(&output.stdout).unwrap();
    assert!(dat.header.is_none());
    let names: Vec<&str> = dat.games.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["baddump", "disk"]);
}

#[test]
fn json_is_an_array_with_one_object_per_input() {
    let output = dat(&["--format", "json", BADDUMP, DISK]);
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json[0]["path"], BADDUMP);
    assert_eq!(json[0]["header"]["name"], "ckmame test db");
    assert_eq!(json[0]["games"][0]["roms"][0]["status"], "baddump");
    assert_eq!(json[1]["path"], DISK);
    assert_eq!(json[1]["games"][0]["disks"][0]["name"], "108-5");

    // Still an array when there is only one.
    let output = dat(&["--format", "json", "--title", "tetris", VIRTUAL_BOY]);
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["games"].as_array().unwrap().len(), 2);
}

#[test]
fn output_goes_to_a_file_when_asked() {
    let dir = std::env::temp_dir().join(format!("datary-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.txt");

    let output = dat(&["--output", path.to_str().unwrap(), BADDUMP]);
    assert!(output.status.success());
    assert_eq!(stdout(&output), "");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        stdout(&dat(&[BADDUMP]))
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_unreadable_input_is_reported_and_the_rest_still_shown() {
    let output = dat(&["--title", "disk", DISK, "missing.dat"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).starts_with("error: missing.dat: "),
        "{}",
        stderr(&output)
    );
    assert!(stdout(&output).contains("\ndisk:\n"));
}

#[test]
fn verbose_reports_on_standard_error() {
    let output = dat(&["-v", "--title", "bad", BADDUMP]);
    assert_eq!(
        stderr(&output),
        format!("read {BADDUMP}: ClrMamePro, 1 game, 1 rom\n{BADDUMP}: matched 1 of 1 game\n")
    );
}

#[test]
fn latin1_input_is_rejected_with_a_tip() {
    let output = run(&[], LATIN1);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "error: <stdin>: invalid UTF-8 at byte 17; the file may be ISO-8859-1\n  \
         tip: pass --latin1 to read it as ISO-8859-1\n"
    );
}

#[test]
fn latin1_input_is_read_when_asked_and_written_as_utf8() {
    let output = run(&["--latin1", "--title", "pokemon strasse"], LATIN1);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Pokémon Straße:\n  description: Pokémon Straße\n"
    );

    let output = run(&["--latin1", "--format", "dat"], LATIN1);
    let dat = datary::from_bytes(&output.stdout).expect("output is UTF-8");
    assert_eq!(dat.games[0].name, "Pokémon Straße");
}

#[test]
fn latin1_leaves_utf8_input_alone() {
    // These bytes are valid Latin-1 too, as every byte string is. Decoding them
    // that way would read `é` (0xC3 0xA9) as `Ã©`.
    let utf8 = "game (\n\tname \"Pokémon\"\n\tdescription \"Pokémon\"\n)\n";
    let output = run(&["--latin1", "-v"], utf8.as_bytes());
    assert_eq!(stdout(&output), "Pokémon:\n  description: Pokémon\n");
    assert_eq!(
        stderr(&output),
        "read <stdin>: ClrMamePro, 1 game, 0 roms\n"
    );
}
