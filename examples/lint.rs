//! Reports referential integrity problems in a datafile.
//!
//! ```sh
//! cargo run --example lint -- tests/fixtures/logiqx/full-dtd.dat
//! ```
//!
//! Exits non-zero when problems are found, so it can gate a pipeline.

use std::collections::BTreeMap;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: lint <datafile.dat>");
        return ExitCode::from(2);
    };

    let dat = match datary::read_file(&path) {
        Ok(dat) => dat,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::from(2);
        }
    };

    let issues = dat.validate();
    if issues.is_empty() {
        println!("{path}: {} games, no integrity problems", dat.games.len());
        return ExitCode::SUCCESS;
    }

    for issue in &issues {
        println!("{issue}");
    }

    // Summarise by kind, so a datafile with thousands of one problem reads well.
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for issue in &issues {
        *counts.entry(kind_name(&issue.kind)).or_default() += 1;
    }

    println!("\n{} problem(s) in {}:", issues.len(), path);
    for (kind, count) in counts {
        println!("  {count:>5}  {kind}");
    }

    ExitCode::FAILURE
}

fn kind_name(kind: &datary::IssueKind) -> &'static str {
    use datary::IssueKind as K;
    match kind {
        K::DuplicateName { .. } => "duplicate game name",
        K::DuplicateId { .. } => "duplicate game id",
        K::UnknownCloneOf { .. } => "dangling cloneof",
        K::UnknownCloneOfId { .. } => "dangling cloneofid",
        K::UnknownRomOf { .. } => "dangling romof",
        K::UnknownSampleOf { .. } => "dangling sampleof",
        K::SelfParent => "self parent",
        K::ConflictingParents { .. } => "conflicting parents",
        K::UnknownMerge { .. } => "dangling merge",
        K::MergeWithoutParent { .. } => "merge without parent",
        K::CloneCycle { .. } => "clone cycle",
        _ => "other",
    }
}
