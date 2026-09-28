//! `dat`: read, convert, search and inspect DAT datafiles.
//!
//! ```sh
//! dat "Nintendo - Virtual Boy.dat"                    # show everything
//! dat --title "mario land" ~/dats/*.dat               # search by title
//! dat --sha1 5177015a91442e56bd76af39447bca365e06c272 ~/dats/*.dat
//! dat --format mame --output vb.dat "Nintendo - Virtual Boy.dat"
//! ```

#![warn(clippy::pedantic)]

mod render;
mod search;

use anstyle::{AnsiColor, Style};
use clap::{ArgGroup, CommandFactory, Parser, ValueEnum};
use datary::format::{ClrMamePro, DatFormat, Xml};
use datary::{Datafile, Md5, Sha1, Sha256, WriteOptions};
use render::{Section, TextOptions};
use search::{Query, Title};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Read, convert, search and inspect DAT datafiles.
///
/// Reads Logiqx XML datafiles (as published by No-Intro, Redump and TOSEC) and
/// ClrMamePro ones (as written by MAME's -listinfo), telling them apart by
/// content. Prints every game, or only those that --sha1, --sha256, --md5 or
/// --title select, as text, JSON, or either datafile syntax.
#[derive(Parser)]
#[command(name = "dat", version, after_help = AFTER_HELP)]
#[command(group(ArgGroup::new("query").args(["sha1", "sha256", "md5", "title"])))]
struct Args {
    /// Report what was read and what matched, on standard error.
    #[arg(short, long)]
    verbose: bool,

    /// Only games with a ROM or disk of this SHA-1.
    #[arg(long, value_name = "SHA1")]
    sha1: Option<Sha1>,

    /// Only games with a ROM of this SHA-256.
    #[arg(long, value_name = "SHA256")]
    sha256: Option<Sha256>,

    /// Only games with a ROM or disk of this MD5.
    #[arg(long, value_name = "MD5")]
    md5: Option<Md5>,

    /// Only games whose name or description contains these words, in order,
    /// ignoring case and accents. "A C" matches "A: b C", and "pokemon"
    /// matches "Pokémon".
    #[arg(long, value_name = "TITLE", value_parser = Title::parse)]
    title: Option<Title>,

    /// Read input that is not valid UTF-8 as ISO-8859-1 (Latin-1), as older
    /// TOSEC and ClrMamePro datafiles often are, rather than rejecting it.
    /// Output is always UTF-8.
    #[arg(long)]
    latin1: bool,

    /// How to write the result. Case-insensitive.
    #[arg(short, long, value_enum, ignore_case = true, default_value_t = Format::Text)]
    format: Format,

    /// Write to this file instead of standard output.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Datafiles to read. Standard input is read when none are given, or
    /// for "-".
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

const AFTER_HELP: &str = "\
DAT and MAME write a single datafile. Given several inputs, they combine their
games and leave out the header, since no one input's header describes the result.

Exit status is 0 on success, 1 when a search matched nothing, and 2 on an error.";

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    /// Indented text, coloured on a terminal.
    Text,
    /// An array holding one object per input.
    Json,
    /// Logiqx XML, as published by No-Intro, Redump and TOSEC.
    #[value(alias = "xml")]
    Dat,
    /// ClrMamePro syntax, as in MAME's -listinfo output.
    #[value(alias = "clrmamepro", alias = "cmpro")]
    Mame,
}

const ERROR: Style = AnsiColor::Red.on_default().bold();

/// Standard input's name in messages and output.
const STDIN: &str = "<stdin>";

fn main() -> ExitCode {
    let args = Args::parse();
    match run(args) {
        Ok(code) => code,
        // The reader went away (`dat big.dat | head`), which is not our error.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            anstream::eprintln!("{ERROR}error:{ERROR:#} {e}");
            ExitCode::from(2)
        }
    }
}

fn run(mut args: Args) -> io::Result<ExitCode> {
    let paths = std::mem::take(&mut args.paths);
    let paths = if paths.is_empty() {
        // Waiting on a terminal for a datafile to be typed in is never what
        // was meant.
        if io::stdin().is_terminal() {
            Args::command()
                .error(
                    clap::error::ErrorKind::MissingRequiredArgument,
                    "no datafile given; pass a PATH, or pipe one to standard input",
                )
                .exit();
        }
        vec![PathBuf::from("-")]
    } else {
        paths
    };

    // The argument group lets through at most one of these.
    let query = [
        args.sha1.take().map(Query::Sha1),
        args.sha256.take().map(Query::Sha256),
        args.md5.take().map(Query::Md5),
        args.title.take().map(Query::Title),
    ]
    .into_iter()
    .flatten()
    .next();

    let (mut sections, failed) = read_all(&paths, &args);
    if sections.is_empty() {
        return Ok(ExitCode::from(2));
    }

    let miss = query
        .as_ref()
        .and_then(|query| search(query, &mut sections, args.verbose));

    let output = render(&args, sections, query.is_some(), paths.len() > 1)?;
    write(&args, &output)?;

    if let Some(miss) = &miss {
        anstream::eprintln!("{miss}");
    }

    Ok(if failed {
        ExitCode::from(2)
    } else if miss.is_some() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Reads every input, reporting the ones that fail rather than stopping at
/// them — like grep, the rest are still searched, and the returned flag lets
/// the exit status record that something went wrong.
fn read_all(paths: &[PathBuf], args: &Args) -> (Vec<Section>, bool) {
    let mut failed = false;
    let mut sections = Vec::new();
    for path in paths {
        let source = if path == Path::new("-") {
            STDIN.to_owned()
        } else {
            path.display().to_string()
        };
        match read(path, args.latin1) {
            Ok((dat, how)) => {
                if args.verbose {
                    anstream::eprintln!(
                        "read {source}: {how}, {}, {}",
                        count(dat.games.len(), "game"),
                        count(dat.rom_count(), "rom")
                    );
                }
                sections.push(Section { source, dat });
            }
            Err(e) => {
                anstream::eprintln!("{ERROR}error:{ERROR:#} {source}: {e}");
                // With --latin1 every byte decodes, so this only happens without it.
                if matches!(e, datary::Error::Encoding { .. }) {
                    anstream::eprintln!("  tip: pass --latin1 to read it as ISO-8859-1");
                }
                failed = true;
            }
        }
    }
    (sections, failed)
}

/// Narrows every section to the games `query` matches. When nothing matched
/// anywhere, returns the message saying so, with a suggestion if one is close.
fn search(query: &Query, sections: &mut [Section], verbose: bool) -> Option<String> {
    let mut matched = 0;
    // Kept only to suggest a title from, should nothing match.
    let mut rejected = Vec::new();
    for section in sections {
        let total = section.dat.games.len();
        let (games, others): (Vec<_>, Vec<_>) = std::mem::take(&mut section.dat.games)
            .into_iter()
            .partition(|game| query.matches(game));
        section.dat.games = games;
        rejected.extend(others);
        matched += section.dat.games.len();
        if verbose {
            anstream::eprintln!(
                "{}: matched {} of {}",
                section.source,
                section.dat.games.len(),
                count(total, "game")
            );
        }
    }

    if matched > 0 {
        return None;
    }
    let suggestion = match query {
        Query::Title(title) => title.suggest(&rejected),
        Query::Sha1(_) | Query::Sha256(_) | Query::Md5(_) => None,
    };
    Some(match suggestion {
        Some(title) => format!("no game matches {query}; did you mean {title:?}?"),
        None => format!("no game matches {query}"),
    })
}

/// Renders the sections in the requested format.
fn render(
    args: &Args,
    sections: Vec<Section>,
    searching: bool,
    several: bool,
) -> io::Result<String> {
    // A search reports only the inputs it found something in.
    let shown = || -> Vec<&Section> {
        sections
            .iter()
            .filter(|s| !searching || !s.dat.games.is_empty())
            .collect()
    };

    Ok(match args.format {
        Format::Text => render::text(
            &shown(),
            &TextOptions {
                color: args.output.is_none(),
                headings: several,
                header: !searching,
            },
        ),
        Format::Json => {
            let mut json = serde_json::to_string_pretty(&render::json(&shown()))?;
            json.push('\n');
            json
        }
        Format::Dat | Format::Mame => {
            if args.verbose && sections.len() > 1 {
                anstream::eprintln!(
                    "combined {} datafiles into one, without a header",
                    sections.len()
                );
            }
            let dat = combine(sections);
            let written = if args.format == Format::Dat {
                Xml.write(&dat, &WriteOptions::default())
            } else {
                ClrMamePro.write(&dat, &WriteOptions::clrmamepro())
            };
            written.map_err(io::Error::other)?
        }
    })
}

/// Writes the rendered output to `--output`, or standard output.
fn write(args: &Args, output: &str) -> io::Result<()> {
    match &args.output {
        Some(path) => std::fs::write(path, output)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display()))),
        // Styles are stripped again here unless standard output is a terminal
        // that wants them; see `anstream` for the environment variables that
        // override it.
        None if args.format == Format::Text => {
            let mut stdout = anstream::stdout().lock();
            stdout.write_all(output.as_bytes())?;
            stdout.flush()
        }
        None => {
            let mut stdout = io::stdout().lock();
            stdout.write_all(output.as_bytes())?;
            stdout.flush()
        }
    }
}

/// Reads a datafile from `path`, or standard input for `-`, returning it with
/// a description of how it was read: its syntax, and its encoding if that was
/// not UTF-8.
///
/// With `latin1`, input that is not valid UTF-8 is decoded as ISO-8859-1. Only
/// then: a UTF-8 file with accents in it is also valid Latin-1, but decoding
/// it that way would turn every `é` into `Ã©`. The reverse mistake is far less
/// likely, as Latin-1 text with accents in it is almost never valid UTF-8.
fn read(path: &Path, latin1: bool) -> datary::Result<(Datafile, String)> {
    let bytes = if path == Path::new("-") {
        let mut bytes = Vec::new();
        io::stdin().lock().read_to_end(&mut bytes)?;
        bytes
    } else {
        std::fs::read(path)?
    };
    // Detection already succeeded inside the parse; this only names it.
    let syntax = || datary::detect(&bytes, datary::BUILTIN_FORMATS).map_or("", |f| f.name());

    match datary::from_bytes(&bytes) {
        Err(datary::Error::Encoding { .. }) if latin1 => {
            let dat = datary::from_str(&datary::decode_latin1(&bytes))?;
            Ok((dat, format!("{}, as ISO-8859-1", syntax())))
        }
        result => Ok((result?, syntax().to_owned())),
    }
}

/// The one datafile that DAT and MAME output need.
///
/// A single input is written as it was read, header and all. Several are
/// combined game by game, without a header, since none of theirs describes
/// the result.
fn combine(mut sections: Vec<Section>) -> Datafile {
    if sections.len() == 1 {
        return sections.remove(0).dat;
    }
    Datafile {
        games: sections.into_iter().flat_map(|s| s.dat.games).collect(),
        ..Datafile::default()
    }
}

/// `1 game`, `2 games`.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}
