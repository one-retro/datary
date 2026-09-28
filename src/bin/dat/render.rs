//! Rendering datafiles as JSON, or as indented and optionally coloured text.
//!
//! Both renderings start from the same JSON view of the model, so the text
//! output can never show a field the JSON leaves out, or vice versa.

use anstyle::{AnsiColor, Style};
use datary::Datafile;
use serde_json::{Map, Value};
use std::fmt::Write as _;

/// One input's datafile, after any query has been applied.
pub struct Section {
    /// Where it was read from, as shown to the user.
    pub source: String,
    /// The datafile, holding only the games that matched.
    pub dat: Datafile,
}

/// A datafile as a JSON object, keyed by the datafile's own attribute and
/// element names.
///
/// This reuses the model's serde derive rather than mirroring every struct, so
/// a field the library gains shows up here without a change. That derive is
/// shaped for quick-xml, though, so two of its conventions are undone: the `@`
/// marking an XML attribute is dropped, and an element that XML repeats once
/// per entry (`rom`) becomes a single plural array (`roms`). The namespace
/// attributes are XML plumbing with no meaning outside it, and are dropped.
pub fn datafile_json(dat: &Datafile) -> Map<String, Value> {
    // Every key is a string and every value a string, number or struct, so
    // this cannot fail short of a bug in the model's derive.
    let value = serde_json::to_value(dat).expect("a datafile serialises to JSON");
    match normalize(value) {
        Value::Object(map) => map,
        _ => unreachable!("a datafile serialises to a JSON object"),
    }
}

fn normalize(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter_map(|(key, value)| {
                    let key = key.strip_prefix('@').unwrap_or(&key);
                    if key.contains(':') {
                        return None;
                    }
                    let key = match &value {
                        Value::Array(_) => plural(key),
                        _ => key.to_owned(),
                    };
                    Some((key, normalize(value)))
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(normalize).collect()),
        scalar => scalar,
    }
}

/// `rom` → `roms`, `category` → `categories`.
fn plural(key: &str) -> String {
    match key.strip_suffix('y') {
        Some(stem) => format!("{stem}ies"),
        None => format!("{key}s"),
    }
}

/// Every section as a JSON array of objects, each naming its source `path`
/// ahead of the datafile's own fields.
///
/// Always an array, even for one input, so that a script reading the output
/// does not have to care how many files it was given.
pub fn json(sections: &[&Section]) -> Value {
    sections
        .iter()
        .map(|section| {
            let mut object = Map::new();
            object.insert("path".to_owned(), Value::String(section.source.clone()));
            object.extend(datafile_json(&section.dat));
            Value::Object(object)
        })
        .collect()
}

/// What to include in text output, and how.
pub struct TextOptions {
    /// Emit ANSI styles. The caller strips them again if the destination turns
    /// out not to be a terminal.
    pub color: bool,
    /// Head each section with its source, for when there are several.
    pub headings: bool,
    /// Show each datafile's header, not just its games.
    pub header: bool,
}

const FILE: Style = Style::new().bold().underline();
const GAME: Style = AnsiColor::Green.on_default().bold();
const KEY: Style = AnsiColor::Cyan.on_default();
const NUMBER: Style = AnsiColor::Yellow.on_default();
const HASH: Style = AnsiColor::Magenta.on_default();
const BAD: Style = AnsiColor::Red.on_default().bold();

/// Renders sections as indented `key: value` text, with each game headed by
/// its name and repeated entries such as ROMs listed YAML-style.
pub fn text(sections: &[&Section], options: &TextOptions) -> String {
    let mut text = Text {
        out: String::new(),
        color: options.color,
        gap: false,
    };

    for section in sections {
        let mut dat = datafile_json(&section.dat);
        // `shift_remove`, as a plain `remove` swaps the last key into the gap.
        let games = match dat.shift_remove("games") {
            Some(Value::Array(games)) => games,
            _ => Vec::new(),
        };

        if options.headings {
            text.gap();
            text.paint(FILE, &format!("==> {} <==", section.source));
            text.out.push('\n');
        }

        // What is left is the header and the datafile's own attributes.
        if options.header && dat.values().any(|v| !is_blank(v)) {
            text.gap();
            text.fields(&dat, 0);
            text.gap = true;
        }

        for game in &games {
            if let Value::Object(game) = game {
                text.game(game);
            }
        }
    }

    text.out
}

struct Text {
    out: String,
    color: bool,
    /// Whether a blank line is owed before the next block.
    gap: bool,
}

impl Text {
    fn gap(&mut self) {
        if std::mem::take(&mut self.gap) {
            self.out.push('\n');
        }
    }

    fn game(&mut self, game: &Map<String, Value>) {
        self.gap();
        let name = game.get("name").and_then(Value::as_str).unwrap_or_default();
        self.paint(GAME, name);
        self.out.push_str(":\n");

        for (key, value) in game.iter().filter(|(key, _)| *key != "name") {
            self.field(2, key, value);
        }
        self.gap = true;
    }

    /// Writes one `key: value` line per field, each starting at `indent`.
    fn fields(&mut self, map: &Map<String, Value>, indent: usize) {
        for (key, value) in map {
            self.field(indent, key, value);
        }
    }

    /// Writes a field on a line of its own, unless it is blank.
    fn field(&mut self, indent: usize, key: &str, value: &Value) {
        if !is_blank(value) {
            self.pad(indent);
            self.entry(indent, key, value);
        }
    }

    /// Writes a field whose first line has already been indented.
    fn entry(&mut self, indent: usize, key: &str, value: &Value) {
        self.paint(KEY, key);
        self.out.push(':');
        match value {
            Value::Object(map) => {
                self.out.push('\n');
                self.fields(map, indent + 2);
            }
            Value::Array(items) => {
                self.out.push('\n');
                for item in items {
                    self.pad(indent + 2);
                    self.item(indent + 2, item);
                }
            }
            scalar => {
                self.out.push(' ');
                self.scalar(key, scalar);
                self.out.push('\n');
            }
        }
    }

    /// Writes one list entry, starting at a line already indented to `indent`.
    fn item(&mut self, indent: usize, value: &Value) {
        self.out.push_str("- ");
        match value {
            Value::Object(map) => {
                // The first field shares the dash's line; the rest align under it.
                let mut fields = map.iter().filter(|(_, v)| !is_blank(v));
                match fields.next() {
                    Some((key, value)) => self.entry(indent + 2, key, value),
                    None => self.out.push('\n'),
                }
                for (key, value) in fields {
                    self.pad(indent + 2);
                    self.entry(indent + 2, key, value);
                }
            }
            Value::Array(items) => {
                self.out.push('\n');
                for item in items {
                    self.pad(indent + 2);
                    self.item(indent + 2, item);
                }
            }
            scalar => {
                self.scalar("", scalar);
                self.out.push('\n');
            }
        }
    }

    fn scalar(&mut self, key: &str, value: &Value) {
        match value {
            Value::String(s) if matches!(key, "crc" | "md5" | "sha1" | "sha256") => {
                self.paint(HASH, s);
            }
            Value::String(s) if key == "status" && !matches!(s.as_str(), "good" | "verified") => {
                self.paint(BAD, s);
            }
            Value::String(s) => self.push_escaped(s),
            other => self.paint(NUMBER, &other.to_string()),
        }
    }

    fn paint(&mut self, style: Style, text: &str) {
        if self.color {
            let _ = write!(self.out, "{style}");
            self.push_escaped(text);
            let _ = write!(self.out, "{style:#}");
        } else {
            self.push_escaped(text);
        }
    }

    /// Appends text with control characters escaped, so that a stray newline
    /// in a name cannot break the layout and an escape sequence in a
    /// datafile cannot reach the terminal.
    fn push_escaped(&mut self, text: &str) {
        for c in text.chars() {
            if c.is_control() {
                self.out.extend(c.escape_default());
            } else {
                self.out.push(c);
            }
        }
    }

    fn pad(&mut self, indent: usize) {
        self.out.extend(std::iter::repeat_n(' ', indent));
    }
}

/// Values not worth a line of text: the model uses an empty string, not
/// `None`, for a missing required element such as a header's `author`.
fn is_blank(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
        <datafile xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
            <header>
                <id>15</id>
                <name>Test</name>
                <description>Test</description>
                <version>1</version>
                <author></author>
            </header>
            <game name="A: b C" id="0001">
                <category>Games</category>
                <category>Preproduction</category>
                <description>A: b C</description>
                <rom name="a.rom" size="3" crc="00000001" status="baddump"/>
                <rom name="b.rom" size="4" md5="d41d8cd98f00b204e9800998ecf8427e"/>
            </game>
        </datafile>"#;

    fn section(source: &str, dat: Datafile) -> Section {
        Section {
            source: source.to_owned(),
            dat,
        }
    }

    #[test]
    fn json_keys_are_the_datafile_names_without_xml_plumbing() {
        let dat = datary::from_str(SAMPLE).unwrap();
        let json = serde_json::to_string(&datafile_json(&dat)).unwrap();
        assert_eq!(
            json,
            concat!(
                r#"{"header":{"id":15,"name":"Test","description":"Test","version":"1","author":""},"#,
                r#""games":[{"name":"A: b C","id":"0001","categories":["Games","Preproduction"],"#,
                r#""description":"A: b C","roms":["#,
                r#"{"name":"a.rom","size":3,"crc":"00000001","status":"baddump"},"#,
                r#"{"name":"b.rom","size":4,"md5":"d41d8cd98f00b204e9800998ecf8427e"}]}]}"#,
            )
        );
    }

    #[test]
    fn json_is_an_array_of_sources() {
        let dat = datary::from_str(SAMPLE).unwrap();
        let sections = [section("a.dat", dat)];
        let json = json(&sections.iter().collect::<Vec<_>>());
        assert_eq!(json[0]["path"], "a.dat");
        assert_eq!(json[0]["games"][0]["roms"][1]["size"], 4);
    }

    #[test]
    fn text_nests_fields_under_each_game() {
        let dat = datary::from_str(SAMPLE).unwrap();
        let sections = [section("a.dat", dat)];
        let options = TextOptions {
            color: false,
            headings: false,
            header: true,
        };
        assert_eq!(
            text(&sections.iter().collect::<Vec<_>>(), &options),
            "\
header:
  id: 15
  name: Test
  description: Test
  version: 1

A: b C:
  id: 0001
  categories:
    - Games
    - Preproduction
  description: A: b C
  roms:
    - name: a.rom
      size: 3
      crc: 00000001
      status: baddump
    - name: b.rom
      size: 4
      md5: d41d8cd98f00b204e9800998ecf8427e
"
        );
    }

    #[test]
    fn text_heads_each_source_when_asked() {
        let a = datary::from_str(
            r#"<datafile><game name="A"><description>A</description></game></datafile>"#,
        )
        .unwrap();
        let b = datary::from_str(
            r#"<datafile><game name="B"><description>B</description></game></datafile>"#,
        )
        .unwrap();
        let sections = [section("a.dat", a), section("b.dat", b)];
        let options = TextOptions {
            color: false,
            headings: true,
            header: false,
        };
        assert_eq!(
            text(&sections.iter().collect::<Vec<_>>(), &options),
            "==> a.dat <==\nA:\n  description: A\n\n==> b.dat <==\nB:\n  description: B\n"
        );
    }

    #[test]
    fn text_styles_only_when_asked() {
        let dat = datary::from_str(SAMPLE).unwrap();
        let sections = [section("a.dat", dat)];
        let render = |color| {
            let options = TextOptions {
                color,
                headings: false,
                header: false,
            };
            text(&sections.iter().collect::<Vec<_>>(), &options)
        };
        assert!(!render(false).contains('\x1b'));
        assert!(render(true).contains(&format!("{GAME}A: b C{GAME:#}")));
        assert!(render(true).contains(&format!("{BAD}baddump{BAD:#}")));
    }

    #[test]
    fn text_escapes_control_characters() {
        let dat = datary::from_str(
            "<datafile><game name=\"Evil&#x1b;[2J\"><description>two&#10;lines</description></game></datafile>",
        )
        .unwrap();
        let sections = [section("a.dat", dat)];
        let options = TextOptions {
            color: false,
            headings: false,
            header: false,
        };
        assert_eq!(
            text(&sections.iter().collect::<Vec<_>>(), &options),
            "Evil\\u{1b}[2J:\n  description: two\\nlines\n"
        );
    }
}
