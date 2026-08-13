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
        out.push_str("clrmamepro (\n");
        field(&mut out, &indent, "name", &header.name);
        field(&mut out, &indent, "description", &header.description);
        if let Some(v) = &header.category {
            field(&mut out, &indent, "category", v);
        }
        field(&mut out, &indent, "version", &header.version);
        if let Some(v) = &header.date {
            field(&mut out, &indent, "date", v);
        }
        field(&mut out, &indent, "author", &header.author);
        if let Some(v) = &header.email {
            field(&mut out, &indent, "email", v);
        }
        if let Some(v) = &header.homepage {
            field(&mut out, &indent, "homepage", v);
        }
        if let Some(v) = &header.url {
            field(&mut out, &indent, "url", v);
        }
        if let Some(v) = &header.comment {
            field(&mut out, &indent, "comment", v);
        }

        if let Some(cmp) = &header.clr_mame_pro {
            if let Some(v) = &cmp.header {
                field(&mut out, &indent, "header", v);
            }
            if let Some(v) = cmp.force_merging {
                field(
                    &mut out,
                    &indent,
                    "forcemerging",
                    &format!("{v:?}").to_lowercase(),
                );
            }
            if let Some(v) = cmp.force_no_dump {
                field(
                    &mut out,
                    &indent,
                    "forcenodump",
                    &format!("{v:?}").to_lowercase(),
                );
            }
            // CMPro spells this `forcezipping`.
            if let Some(v) = cmp.force_packing {
                field(
                    &mut out,
                    &indent,
                    "forcezipping",
                    &format!("{v:?}").to_lowercase(),
                );
            }
        }

        out.push_str(")\n\n");
    }

    for game in &dat.games {
        out.push_str("game (\n");
        field(&mut out, &indent, "name", &game.name);
        field(&mut out, &indent, "description", &game.description);
        if let Some(v) = &game.year {
            field(&mut out, &indent, "year", v);
        }
        if let Some(v) = &game.manufacturer {
            field(&mut out, &indent, "manufacturer", v);
        }
        if let Some(v) = &game.clone_of {
            field(&mut out, &indent, "cloneof", v);
        }
        if let Some(v) = &game.rom_of {
            field(&mut out, &indent, "romof", v);
        }
        if let Some(v) = &game.sample_of {
            field(&mut out, &indent, "sampleof", v);
        }

        for rom in &game.roms {
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
            write_flags(&mut out, rom.status());
            out.push_str(" )\n");
        }

        for disk in &game.disks {
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
            write_flags(&mut out, disk.status());
            out.push_str(" )\n");
        }

        for sample in &game.samples {
            writeln!(out, "{indent}sample ( name {} )", quote(&sample.name)).unwrap();
        }

        out.push_str(")\n\n");
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
