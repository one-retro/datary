//! Tests for referential integrity checking.

use datary::{Datafile, IssueKind};
use pretty_assertions::assert_eq;
use std::path::{Path, PathBuf};

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn parse(xml: &str) -> Datafile {
    datary::from_str(xml).expect("fixture should parse")
}

/// Wraps games in a datafile.
fn dat(games: &str) -> Datafile {
    parse(&format!("<datafile>{games}</datafile>"))
}

fn game(attrs: &str) -> String {
    format!("<game {attrs}><description>d</description></game>")
}

#[test]
fn a_sound_datafile_has_no_issues() {
    let d = dat(&format!(
        "{}{}",
        game(r#"name="Parent" id="0001""#),
        game(r#"name="Child" cloneof="Parent" cloneofid="0001""#)
    ));
    assert_eq!(d.validate(), vec![]);
}

/// Every published fixture must be referentially sound.
#[test]
fn published_fixtures_are_sound() {
    for name in [
        "no-intro/virtual-boy.dat",
        "no-intro/famicom-network-system.dat",
        "no-intro/new-nintendo-3ds-decrypted.dat",
        "no-intro/pokemon-mini.dat",
        "tosec/nes-games.dat",
        "logiqx/full-dtd.dat",
    ] {
        let d = datary::read_file(fixture(name)).unwrap();
        let issues = d.validate();
        assert!(
            issues.is_empty(),
            "{name} reported {} issue(s): {}",
            issues.len(),
            issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
}

#[test]
fn a_dangling_cloneof_is_reported() {
    let d = dat(&game(r#"name="Child" cloneof="Missing""#));
    let issues = d.validate();

    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].kind,
        IssueKind::UnknownCloneOf {
            name: "Missing".into()
        }
    );
    assert_eq!(issues[0].game, 0);
    assert_eq!(issues[0].game_name, "Child");
    assert!(issues[0].to_string().contains("Missing"), "{}", issues[0]);
}

#[test]
fn a_dangling_cloneofid_is_reported() {
    let d = dat(&game(r#"name="Child" cloneofid="9999""#));
    assert_eq!(
        d.validate()[0].kind,
        IssueKind::UnknownCloneOfId { id: "9999".into() }
    );
}

#[test]
fn dangling_romof_and_sampleof_are_reported() {
    let d = dat(&game(r#"name="G" romof="NoRom" sampleof="NoSample""#));
    let kinds: Vec<_> = d.validate().into_iter().map(|i| i.kind).collect();

    assert!(kinds.contains(&IssueKind::UnknownRomOf {
        name: "NoRom".into()
    }));
    assert!(kinds.contains(&IssueKind::UnknownSampleOf {
        name: "NoSample".into()
    }));
}

#[test]
fn duplicate_names_are_reported_against_the_later_game() {
    let d = dat(&format!(
        "{}{}",
        game(r#"name="Same""#),
        game(r#"name="Same""#)
    ));
    let issues = d.validate();

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].game, 1, "the second game is the offender");
    assert_eq!(
        issues[0].kind,
        IssueKind::DuplicateName {
            name: "Same".into(),
            first: 0
        }
    );
}

#[test]
fn duplicate_ids_are_reported() {
    let d = dat(&format!(
        "{}{}",
        game(r#"name="A" id="0001""#),
        game(r#"name="B" id="0001""#)
    ));
    let issues = d.validate();

    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].kind,
        IssueKind::DuplicateId {
            id: "0001".into(),
            first: 0
        }
    );
}

/// Games without an id must not collide with each other.
#[test]
fn absent_ids_do_not_collide() {
    let d = dat(&format!("{}{}", game(r#"name="A""#), game(r#"name="B""#)));
    assert_eq!(d.validate(), vec![]);
}

#[test]
fn a_self_parent_is_reported() {
    let d = dat(&game(r#"name="Loop" cloneof="Loop""#));
    let kinds: Vec<_> = d.validate().into_iter().map(|i| i.kind).collect();
    assert!(kinds.contains(&IssueKind::SelfParent), "{kinds:?}");
}

#[test]
fn conflicting_parents_are_reported() {
    let d = dat(&format!(
        "{}{}{}",
        game(r#"name="ByName" id="0001""#),
        game(r#"name="ById" id="0002""#),
        game(r#"name="Child" cloneof="ByName" cloneofid="0002""#)
    ));
    let issues = d.validate();

    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].kind,
        IssueKind::ConflictingParents {
            by_name: "ByName".into(),
            by_id: "ById".into()
        }
    );
}

/// Agreeing references are not a conflict.
#[test]
fn matching_parents_are_not_a_conflict() {
    let d = dat(&format!(
        "{}{}",
        game(r#"name="Parent" id="0001""#),
        game(r#"name="Child" cloneof="Parent" cloneofid="0001""#)
    ));
    assert_eq!(d.validate(), vec![]);
}

#[test]
fn a_two_game_cycle_is_reported_once() {
    let d = dat(&format!(
        "{}{}",
        game(r#"name="A" cloneof="B""#),
        game(r#"name="B" cloneof="A""#)
    ));
    let cycles: Vec<_> = d
        .validate()
        .into_iter()
        .filter(|i| matches!(i.kind, IssueKind::CloneCycle { .. }))
        .collect();

    assert_eq!(cycles.len(), 1, "a cycle must not be reported per member");
    let IssueKind::CloneCycle { path } = &cycles[0].kind else {
        unreachable!()
    };
    assert_eq!(path.first(), path.last(), "the path closes the loop");
    assert!(path.contains(&"A".to_string()) && path.contains(&"B".to_string()));
}

#[test]
fn a_three_game_cycle_is_reported_once() {
    let d = dat(&format!(
        "{}{}{}",
        game(r#"name="A" cloneof="C""#),
        game(r#"name="B" cloneof="A""#),
        game(r#"name="C" cloneof="B""#)
    ));
    let cycles: Vec<_> = d
        .validate()
        .into_iter()
        .filter(|i| matches!(i.kind, IssueKind::CloneCycle { .. }))
        .collect();
    assert_eq!(cycles.len(), 1, "got {cycles:?}");
}

/// A long parent chain is not a cycle and must terminate.
#[test]
fn a_deep_chain_is_not_a_cycle() {
    let games: String = (0..200)
        .map(|i| {
            if i == 0 {
                game(r#"name="g0""#)
            } else {
                game(&format!(r#"name="g{i}" cloneof="g{}""#, i - 1))
            }
        })
        .collect();
    assert_eq!(dat(&games).validate(), vec![]);
}

#[test]
fn a_merge_naming_a_missing_parent_file_is_reported() {
    let d = parse(
        r#"<datafile>
             <game name="Parent"><description>d</description>
               <rom name="present.rom" size="1"/>
             </game>
             <game name="Child" cloneof="Parent"><description>d</description>
               <rom name="child.rom" size="1" merge="absent.rom"/>
             </game>
           </datafile>"#,
    );
    let issues = d.validate();

    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].kind,
        IssueKind::UnknownMerge {
            file: "child.rom".into(),
            merge: "absent.rom".into(),
            parent: "Parent".into()
        }
    );
}

#[test]
fn a_merge_resolving_into_the_parent_is_accepted() {
    let d = parse(
        r#"<datafile>
             <game name="Parent"><description>d</description>
               <rom name="shared.rom" size="1"/>
             </game>
             <game name="Child" cloneof="Parent"><description>d</description>
               <rom name="child.rom" size="1" merge="shared.rom"/>
             </game>
           </datafile>"#,
    );
    assert_eq!(d.validate(), vec![]);
}

/// `romof` takes precedence over `cloneof` when resolving a merge target.
#[test]
fn merges_resolve_through_romof_first() {
    let d = parse(
        r#"<datafile>
             <game name="RomParent"><description>d</description>
               <rom name="from-romof.rom" size="1"/>
             </game>
             <game name="CloneParent"><description>d</description>
               <rom name="from-cloneof.rom" size="1"/>
             </game>
             <game name="Child" cloneof="CloneParent" romof="RomParent"><description>d</description>
               <rom name="c.rom" size="1" merge="from-romof.rom"/>
             </game>
           </datafile>"#,
    );
    assert_eq!(d.validate(), vec![], "romof should have been searched");
}

#[test]
fn a_merge_without_a_parent_is_reported() {
    let d = parse(
        r#"<datafile>
             <game name="Orphan"><description>d</description>
               <rom name="a.rom" size="1" merge="somewhere.rom"/>
             </game>
           </datafile>"#,
    );
    assert_eq!(
        d.validate()[0].kind,
        IssueKind::MergeWithoutParent {
            file: "a.rom".into(),
            merge: "somewhere.rom".into()
        }
    );
}

#[test]
fn disk_merges_are_checked_too() {
    let d = parse(
        r#"<datafile>
             <game name="Parent"><description>d</description>
               <disk name="present"/>
             </game>
             <game name="Child" cloneof="Parent"><description>d</description>
               <disk name="child" merge="absent"/>
             </game>
           </datafile>"#,
    );
    assert!(matches!(
        d.validate()[0].kind,
        IssueKind::UnknownMerge { .. }
    ));
}

#[test]
fn several_problems_are_all_reported() {
    let d = dat(&format!(
        "{}{}{}",
        game(r#"name="Dup""#),
        game(r#"name="Dup" cloneof="Nowhere""#),
        game(r#"name="Third" cloneofid="nope" romof="Absent""#)
    ));
    let issues = d.validate();

    assert!(
        issues.len() >= 4,
        "expected every problem, got {}: {issues:?}",
        issues.len()
    );
}

#[test]
fn an_empty_datafile_is_sound() {
    assert_eq!(Datafile::default().validate(), vec![]);
}

/// A self-parent is reported once, as `SelfParent` — not also as a
/// one-element clone cycle, which would be the same defect twice.
#[test]
fn a_self_parent_is_not_also_reported_as_a_cycle() {
    let d = dat(&game(r#"name="Loop" cloneof="Loop""#));
    let issues = d.validate();

    assert_eq!(issues.len(), 1, "got {issues:?}");
    assert_eq!(issues[0].kind, IssueKind::SelfParent);
}

/// The same holds for a self-reference through `cloneofid`.
#[test]
fn a_self_parent_by_id_is_reported_once() {
    let d = dat(&game(r#"name="Loop" id="0001" cloneofid="0001""#));
    let issues = d.validate();

    assert_eq!(issues.len(), 1, "got {issues:?}");
    assert_eq!(issues[0].kind, IssueKind::SelfParent);
}
