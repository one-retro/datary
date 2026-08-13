//! Writing a [`Datafile`] in ClrMamePro syntax.

use crate::dat::Datafile;
use crate::enums::Status;
use crate::write::WriteOptions;
use std::fmt::Write as _;

/// Quotes a value only when it needs it, matching ClrMamePro's own habit of
/// leaving simple tokens bare.
///
/// The format defines no escape sequence, so an embedded `"` cannot be
/// represented. It is rewritten as `'`, which is lossy but keeps the output
/// parseable — the alternative is emitting a file nothing can read back.
fn quote(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || c == '(' || c == ')' || c == '"');

    if needs_quotes {
        format!("\"{}\"", value.replace('"', "'"))
    } else {
        value.to_string()
    }
}

/// Serialises a datafile in ClrMamePro syntax.
pub fn to_string(dat: &Datafile, options: &WriteOptions) -> String {
    let indent: String = match options.indent {
        Some((character, count)) => std::iter::repeat_n(character, count).collect(),
        None => String::new(),
    };
    let mut out = String::with_capacity(256 + dat.games.len() * 128);

    if let Some(header) = &dat.header {
        write_header(&mut out, &indent, header);
    }
    for game in &dat.games {
        write_game(&mut out, &indent, game);
    }

    // The blank line after the final block is an artefact of the loop.
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !options.trailing_newline {
        while out.ends_with('\n') {
            out.pop();
        }
    }

    if options.line_ending == crate::write::LineEnding::Crlf {
        out = out.replace('\n', "\r\n");
    }

    out
}

fn write_header(out: &mut String, indent: &str, header: &crate::dat::Header) {
    out.push_str("clrmamepro (\n");
    field(out, indent, "name", &header.name);
    field(out, indent, "description", &header.description);
    optional(out, indent, "category", header.category.as_ref());
    field(out, indent, "version", &header.version);
    optional(out, indent, "date", header.date.as_ref());
    field(out, indent, "author", &header.author);
    optional(out, indent, "email", header.email.as_ref());
    optional(out, indent, "homepage", header.homepage.as_ref());
    optional(out, indent, "url", header.url.as_ref());
    optional(out, indent, "comment", header.comment.as_ref());

    if let Some(cmp) = &header.clr_mame_pro {
        optional(out, indent, "header", cmp.header.as_ref());
        if let Some(v) = cmp.force_merging {
            field(out, indent, "forcemerging", &lowercase(v));
        }
        if let Some(v) = cmp.force_no_dump {
            field(out, indent, "forcenodump", &lowercase(v));
        }
        // CMPro spells this `forcezipping`.
        if let Some(v) = cmp.force_packing {
            field(out, indent, "forcezipping", &lowercase(v));
        }
    }

    out.push_str(")\n\n");
}

fn write_game(out: &mut String, indent: &str, game: &crate::dat::Game) {
    out.push_str("game (\n");
    field(out, indent, "name", &game.name);
    field(out, indent, "description", &game.description);
    optional(out, indent, "year", game.year.as_ref());
    optional(out, indent, "manufacturer", game.manufacturer.as_ref());
    optional(out, indent, "cloneof", game.clone_of.as_ref());
    optional(out, indent, "romof", game.rom_of.as_ref());
    optional(out, indent, "sampleof", game.sample_of.as_ref());

    for rom in &game.roms {
        write_rom(out, indent, rom);
    }
    for disk in &game.disks {
        write_disk(out, indent, disk);
    }
    for sample in &game.samples {
        writeln!(out, "{indent}sample ( name {} )", quote(&sample.name)).unwrap();
    }

    out.push_str(")\n\n");
}

fn write_rom(out: &mut String, indent: &str, rom: &crate::dat::Rom) {
    write!(
        out,
        "{indent}rom ( name {} size {}",
        quote(&rom.name),
        rom.size
    )
    .unwrap();
    if let Some(v) = rom.crc {
        write!(out, " crc {v}").unwrap();
    }
    if let Some(v) = rom.md5 {
        write!(out, " md5 {v}").unwrap();
    }
    if let Some(v) = rom.sha1 {
        write!(out, " sha1 {v}").unwrap();
    }
    if let Some(v) = rom.sha256 {
        write!(out, " sha256 {v}").unwrap();
    }
    if let Some(v) = &rom.merge {
        write!(out, " merge {}", quote(v)).unwrap();
    }
    if let Some(v) = &rom.serial {
        write!(out, " serial {}", quote(v)).unwrap();
    }
    if let Some(v) = &rom.date {
        write!(out, " date {}", quote(v)).unwrap();
    }
    write_flags(out, rom.status());
    out.push_str(" )\n");
}

fn write_disk(out: &mut String, indent: &str, disk: &crate::dat::Disk) {
    write!(out, "{indent}disk ( name {}", quote(&disk.name)).unwrap();
    if let Some(v) = disk.md5 {
        write!(out, " md5 {v}").unwrap();
    }
    if let Some(v) = disk.sha1 {
        write!(out, " sha1 {v}").unwrap();
    }
    if let Some(v) = &disk.merge {
        write!(out, " merge {}", quote(v)).unwrap();
    }
    write_flags(out, disk.status());
    out.push_str(" )\n");
}

/// The spec tokens are the enum names lowercased.
fn lowercase(value: impl std::fmt::Debug) -> String {
    format!("{value:?}").to_lowercase()
}

/// Writes `key value` only when the value is present.
fn optional(out: &mut String, indent: &str, key: &str, value: Option<&String>) {
    if let Some(v) = value {
        field(out, indent, key, v);
    }
}

fn field(out: &mut String, indent: &str, key: &str, value: &str) {
    writeln!(out, "{indent}{key} {}", quote(value)).unwrap();
}

/// `good` is the default and is left implicit, matching what producers emit.
fn write_flags(out: &mut String, status: Status) {
    match status {
        Status::Good => {}
        Status::BadDump => out.push_str(" flags baddump"),
        Status::NoDump => out.push_str(" flags nodump"),
        Status::Verified => out.push_str(" flags verified"),
    }
}
