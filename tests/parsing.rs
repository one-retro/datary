//! Tests for parsing edge cases, malformed input, and building datafiles by hand.

use datary::hash::HashParseError;
use datary::{Datafile, Error, Game, Rom, Status, WriteOptions, YesNo};
use pretty_assertions::assert_eq;

#[test]
fn a_minimal_datafile_parses() {
    let dat = datary::from_str("<datafile></datafile>").unwrap();
    assert!(dat.header.is_none());
    assert!(dat.games.is_empty());
    assert_eq!(dat, Datafile::default());
}

#[test]
fn a_game_without_roms_parses() {
    let dat = datary::from_str(
        "<datafile><game name=\"g\"><description>d</description></game></datafile>",
    )
    .unwrap();
    assert_eq!(dat.games.len(), 1);
    assert!(dat.games[0].roms.is_empty());
    assert_eq!(dat.games[0].description, "d");
}

#[test]
fn element_order_does_not_matter() {
    // `release` before `rom` (Logiqx order) and after it (No-Intro order) must
    // both parse to the same thing.
    let logiqx = datary::from_str(
        r#"<datafile><game name="g"><description>d</description>
             <release name="g" region="EU"/>
             <rom name="g.rom" size="1"/>
           </game></datafile>"#,
    )
    .unwrap();
    let no_intro = datary::from_str(
        r#"<datafile><game name="g"><description>d</description>
             <rom name="g.rom" size="1"/>
             <release name="g" region="EU"/>
           </game></datafile>"#,
    )
    .unwrap();

    assert_eq!(logiqx, no_intro);
    assert_eq!(logiqx.games[0].roms.len(), 1);
    assert_eq!(logiqx.games[0].releases.len(), 1);
}

#[test]
fn unknown_elements_and_attributes_are_ignored() {
    // Forward compatibility: a future No-Intro extension must not break parsing.
    let dat = datary::from_str(
        r#"<datafile><game name="g" futureattr="x"><description>d</description>
             <somethingnew>value</somethingnew>
             <rom name="g.rom" size="1" alsonew="y"/>
           </game></datafile>"#,
    )
    .unwrap();

    assert_eq!(dat.games[0].name, "g");
    assert_eq!(dat.games[0].roms[0].name, "g.rom");
}

#[test]
fn xml_entities_are_decoded_and_re_encoded() {
    let dat = datary::from_str(
        r#"<datafile><game name="Tom &amp; Jerry &lt;1&gt;"><description>&quot;quoted&quot;</description>
             <rom name="a&amp;b.rom" size="1"/>
           </game></datafile>"#,
    )
    .unwrap();

    assert_eq!(dat.games[0].name, "Tom & Jerry <1>");
    assert_eq!(dat.games[0].description, "\"quoted\"");
    assert_eq!(dat.games[0].roms[0].name, "a&b.rom");

    // Writing must escape them again, and reparsing must give the same value.
    let xml = datary::to_string(&dat).unwrap();
    assert!(xml.contains("Tom &amp; Jerry"));
    assert_eq!(datary::from_str(&xml).unwrap(), dat);
}

#[test]
fn non_ascii_names_survive_a_round_trip() {
    let dat = datary::from_str(
        r#"<datafile><game name="ポケモン (Japan)"><description>ポケモン</description>
             <rom name="ポケモン.gb" size="1"/>
           </game></datafile>"#,
    )
    .unwrap();

    assert_eq!(dat.games[0].name, "ポケモン (Japan)");
    assert_eq!(
        datary::from_str(&datary::to_string(&dat).unwrap()).unwrap(),
        dat
    );
}

#[test]
fn malformed_xml_is_an_error() {
    let err = datary::from_str("<datafile><game></datafile>").unwrap_err();
    assert!(matches!(err, Error::Xml(_)), "got {err:?}");
}

#[test]
fn a_bad_checksum_is_rejected_with_a_useful_message() {
    // Too short for a SHA-1.
    let err = datary::from_str(
        r#"<datafile><game name="g"><description>d</description>
             <rom name="g.rom" size="1" sha1="abcd"/>
           </game></datafile>"#,
    )
    .unwrap_err();

    let message = err.to_string();
    assert!(
        message.contains("40"),
        "should say the expected width: {message}"
    );
    assert!(
        message.contains('4'),
        "should say the actual width: {message}"
    );
}

#[test]
fn a_non_hex_checksum_is_rejected() {
    let err = datary::from_str(
        r#"<datafile><game name="g"><description>d</description>
             <rom name="g.rom" size="1" crc="zzzzzzzz"/>
           </game></datafile>"#,
    )
    .unwrap_err();
    assert!(matches!(err, Error::Xml(_)));
}

#[test]
fn an_invalid_enum_token_is_rejected() {
    let err = datary::from_str(
        r#"<datafile><game name="g"><description>d</description>
             <rom name="g.rom" size="1" status="sideways"/>
           </game></datafile>"#,
    )
    .unwrap_err();
    assert!(matches!(err, Error::Xml(_)));
}

#[test]
fn a_missing_file_is_an_io_error() {
    let err = datary::read_file("does/not/exist.dat").unwrap_err();
    assert!(matches!(err, Error::Io(_)), "got {err:?}");
}

#[test]
fn hash_parse_errors_convert_into_the_crate_error() {
    let err: Error = HashParseError::InvalidLength {
        expected: 40,
        actual: 4,
    }
    .into();
    assert!(matches!(err, Error::Hash(_)));
    assert!(err.to_string().contains("invalid checksum"));
}

#[test]
fn a_datafile_can_be_built_from_scratch_and_written() {
    let dat = Datafile {
        header: Some(datary::Header {
            name: "My Collection".into(),
            description: "Hand-built".into(),
            version: "1.0".into(),
            author: "me".into(),
            ..datary::Header::default()
        }),
        games: vec![Game {
            name: "Test Game".into(),
            description: "Test Game".into(),
            categories: vec!["Games".into()],
            roms: vec![Rom {
                name: "test.rom".into(),
                size: 1024,
                crc: Some("deadbeef".parse().unwrap()),
                status: Some(Status::Verified),
                mia: Some(YesNo::Yes),
                ..Rom::default()
            }],
            ..Game::default()
        }],
        ..Datafile::default()
    };

    let xml = datary::to_string(&dat).unwrap();
    assert!(xml.contains(r#"<game name="Test Game">"#));
    assert!(xml.contains(r#"crc="deadbeef""#));
    assert!(xml.contains(r#"status="verified""#));
    assert!(xml.contains(r#"mia="yes""#));
    assert!(xml.contains("<category>Games</category>"));

    // Absent optional fields must not be written at all.
    assert!(!xml.contains("cloneof"), "empty optionals must be omitted");
    assert!(!xml.contains("<year>"));
    assert!(!xml.contains("sha256"));

    assert_eq!(datary::from_str(&xml).unwrap(), dat);
}

#[test]
fn writing_to_a_file_round_trips() {
    let dir = std::env::temp_dir().join("datary-write-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.dat");

    let dat = datary::from_str(
        r#"<datafile><game name="g"><description>d</description>
             <rom name="g.rom" size="7" crc="00000007"/>
           </game></datafile>"#,
    )
    .unwrap();

    datary::write_file(&path, &dat).unwrap();
    assert_eq!(datary::read_file(&path).unwrap(), dat);

    datary::write_file_with(&path, &dat, &WriteOptions::no_intro()).unwrap();
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("\r\n"), "no_intro options must write CRLF");
    assert_eq!(datary::read_file(&path).unwrap(), dat);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn readers_and_strings_agree() {
    let xml = r#"<datafile><game name="g"><description>d</description>
                   <rom name="g.rom" size="1"/>
                 </game></datafile>"#;
    assert_eq!(
        datary::from_str(xml).unwrap(),
        datary::from_reader(xml.as_bytes()).unwrap()
    );
}

#[test]
fn a_size_of_zero_is_valid() {
    // nodump entries record no size at all in some datafiles.
    let dat = datary::from_str(
        r#"<datafile><game name="g"><description>d</description>
             <rom name="g.rom" status="nodump"/>
           </game></datafile>"#,
    )
    .unwrap();
    assert_eq!(dat.games[0].roms[0].size, 0);
    assert!(dat.games[0].roms[0].is_no_dump());
}
