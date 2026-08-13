//! The ClrMamePro grammar, parsed with winnow.
//!
//! The whole syntax is four rules:
//!
//! ```text
//! block := IDENT '(' entry* ')'
//! entry := IDENT (value | block)
//! value := BARE | '"' ... '"'
//! BARE  := any run without whitespace, parens or quotes
//! ```
//!
//! There is no published grammar — the format is whatever ClrMamePro accepts —
//! so parsing is deliberately permissive about layout and about which keys
//! appear, and strict only about structure.

use super::ir::{Block, Entry};
use crate::error::CmproError;
use winnow::combinator::{alt, cut_err, preceded, repeat, terminated};
use winnow::error::{ContextError, StrContext, StrContextValue};
use winnow::token::{take_till, take_while};
use winnow::{ModalResult, Parser};

/// Whitespace, plus the bare `BEGIN`/`END` markers some producers wrap files in.
fn ws(input: &mut &str) -> ModalResult<()> {
    repeat(
        0..,
        alt((
            take_while(1.., (' ', '\t', '\r', '\n')).void(),
            "BEGIN".void(),
            "END".void(),
        )),
    )
    .parse_next(input)
}

/// A bare token: any run without whitespace, parens or quotes.
fn bare<'a>(input: &mut &'a str) -> ModalResult<&'a str> {
    take_while(1.., |c: char| {
        !c.is_whitespace() && c != '(' && c != ')' && c != '"'
    })
    .parse_next(input)
}

/// A double-quoted string.
///
/// The format defines no escape sequence, so the body is a contiguous slice and
/// this stays zero-copy.
///
/// The body deliberately stops at a newline as well as at the closing quote.
/// No producer emits a value spanning lines — and without escapes the format
/// could not represent one unambiguously anyway — while allowing it would make
/// a single unterminated quote swallow the rest of the file and report the
/// error thousands of lines below the actual mistake.
///
/// `cut_err` commits once the opening quote is seen, so the failure is reported
/// here rather than backtracking and resurfacing somewhere unrelated.
fn quoted<'a>(input: &mut &'a str) -> ModalResult<&'a str> {
    preceded(
        '"',
        cut_err(terminated(take_till(0.., ('"', '\n')), '"'))
            .context(StrContext::Label("quoted string"))
            .context(StrContext::Expected(StrContextValue::Description(
                "a closing '\"'",
            ))),
    )
    .parse_next(input)
}

fn value<'a>(input: &mut &'a str) -> ModalResult<&'a str> {
    alt((quoted, bare))
        .context(StrContext::Label("value"))
        .parse_next(input)
}

/// `key value` or `key ( ... )`.
fn entry<'a>(input: &mut &'a str) -> ModalResult<Entry<'a>> {
    let key = terminated(bare, ws).parse_next(input)?;
    alt((
        block_body.map(move |entries| Entry::Block(Block { name: key, entries })),
        terminated(value, ws).map(move |v| Entry::Field(key, v)),
    ))
    .parse_next(input)
}

/// The `( ... )` of a block, plus trailing whitespace. Committed after the
/// opening paren so an unbalanced block is reported at the token that broke it.
fn block_body<'a>(input: &mut &'a str) -> ModalResult<Vec<Entry<'a>>> {
    preceded(
        ('(', ws),
        cut_err(terminated(repeat(0.., entry), (')', ws)))
            .context(StrContext::Label("block"))
            .context(StrContext::Expected(StrContextValue::Description(
                "an entry or a closing ')'",
            ))),
    )
    .parse_next(input)
}

/// A top-level `name ( ... )` block.
fn block<'a>(input: &mut &'a str) -> ModalResult<Block<'a>> {
    let name = terminated(bare, ws)
        .context(StrContext::Label("block name"))
        .parse_next(input)?;
    let entries = block_body.parse_next(input)?;
    Ok(Block { name, entries })
}

/// Parses a datafile into its block tree.
///
/// # Errors
///
/// Returns [`CmproError`] with the line, column and a description of what was
/// expected.
pub fn blocks(source: &str) -> Result<Vec<Block<'_>>, CmproError> {
    let mut input = source;
    preceded(ws, repeat(0.., block))
        .parse(&mut input)
        .map_err(|e| to_error(source, &e))
}

fn to_error(source: &str, err: &winnow::error::ParseError<&str, ContextError>) -> CmproError {
    let offset = err.offset();
    let ctx = err.inner();

    let expected: Vec<String> = ctx
        .context()
        .filter_map(|c| match c {
            StrContext::Expected(v) => Some(v.to_string()),
            _ => None,
        })
        .collect();
    let label = ctx.context().find_map(|c| match c {
        StrContext::Label(l) => Some(*l),
        _ => None,
    });

    let message = match (label, expected.is_empty()) {
        (Some(l), false) => format!("invalid {}, expected {}", l, expected.join(" or ")),
        (Some(l), true) => format!("invalid {l}"),
        (None, false) => format!("expected {}", expected.join(" or ")),
        (None, true) if offset >= source.len() => "unexpected end of input".to_string(),
        (None, true) => "unexpected input".to_string(),
    };

    let (line, column) = line_column(source, offset);
    CmproError {
        message,
        line,
        column,
    }
}

/// Converts a byte offset into a 1-based line and column.
fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    // Count characters rather than bytes so non-ASCII names report sensibly.
    let column = source[line_start..offset].chars().count() + 1;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_minimal_block() {
        let blocks = blocks("clrmamepro ( name Test )").unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "clrmamepro");
        assert_eq!(blocks[0].field("name"), Some("Test"));
    }

    #[test]
    fn parses_quoted_values_with_spaces() {
        let blocks = blocks(r#"game ( description "one four byte file" )"#).unwrap();
        assert_eq!(blocks[0].field("description"), Some("one four byte file"));
    }

    #[test]
    fn parses_nested_blocks() {
        let src = "game (\n\tname g\n\trom ( name a.rom size 4 crc deadbeef )\n)";
        let blocks = blocks(src).unwrap();
        let rom = blocks[0].blocks("rom").next().unwrap();
        assert_eq!(rom.field("name"), Some("a.rom"));
        assert_eq!(rom.field("crc"), Some("deadbeef"));
    }

    #[test]
    fn ignores_begin_and_end_markers() {
        let with = blocks("BEGIN\nclrmamepro ( name T )\nEND\n").unwrap();
        let without = blocks("clrmamepro ( name T )").unwrap();
        assert_eq!(with, without);
    }

    #[test]
    fn empty_input_yields_no_blocks() {
        assert!(blocks("").unwrap().is_empty());
        assert!(blocks("  \n\t ").unwrap().is_empty());
    }

    #[test]
    fn an_unbalanced_block_is_reported() {
        let err = blocks("game (\n\tname g\n").unwrap_err();
        assert!(err.message.contains(')'), "{}", err.message);
    }

    #[test]
    fn garbage_is_reported_at_the_start() {
        let err = blocks("%%%\nclrmamepro ( name T )").unwrap_err();
        assert_eq!((err.line, err.column), (1, 1));
    }

    #[test]
    fn line_and_column_count_characters_not_bytes() {
        // 'é' is two bytes, so byte offset 3 is the third *character*.
        // Counting bytes would wrongly report column 4.
        assert_eq!(line_column("aéb\nx", 3), (1, 3));
        // Offset 5 is past the newline.
        assert_eq!(line_column("aéb\nx", 5), (2, 1));
    }

    #[test]
    fn an_unterminated_quote_does_not_swallow_the_rest_of_the_file() {
        let src = "game (\n\tname \"unterminated\n)\n";
        let err = blocks(src).unwrap_err();
        assert_eq!(err.line, 2, "must report the line the quote opened on");
        assert!(err.message.contains("closing"), "{}", err.message);
    }
}
