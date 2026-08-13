//! Referential integrity checks over a parsed datafile.
//!
//! Parsing is deliberately permissive: a datafile whose `cloneof` names a game
//! that is not in the file is still well-formed, still parses, and is still
//! useful for everything that does not follow that link. So these checks are
//! *reported*, never fatal — [`Datafile::validate`] returns findings rather
//! than failing.
//!
//! ```
//! let dat = datary::from_str(r#"
//!     <datafile>
//!         <game name="Child" cloneof="Missing"><description>Child</description></game>
//!     </datafile>"#)?;
//!
//! let issues = dat.validate();
//! assert_eq!(issues.len(), 1);
//! println!("{}", issues[0]);  // game "Child": cloneof names an unknown game "Missing"
//! # Ok::<(), datary::Error>(())
//! ```
//!
//! # Scope
//!
//! This checks *referential integrity only*: that names and ids referenced
//! within the file resolve, that the keys they resolve through are unique, and
//! that the clone graph is acyclic. It does not police conventions — a
//! `description` differing from a `name`, a missing `year`, or a `nodump` entry
//! that nonetheless carries checksums are all left alone, because datafiles in
//! the wild do those deliberately.

use crate::dat::{Datafile, Game};
use std::collections::HashMap;
use std::fmt;

/// A referential integrity problem found by [`Datafile::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Index into [`Datafile::games`] of the game carrying the problem.
    pub game: usize,

    /// The name of that game, so the issue reads on its own.
    pub game_name: String,

    /// What is wrong.
    pub kind: IssueKind,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "game {:?}: {}", self.game_name, self.kind)
    }
}

/// The kind of referential integrity problem.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IssueKind {
    /// Two or more games share a `name`, which is the key `cloneof`,
    /// `romof` and `sampleof` resolve through.
    DuplicateName {
        /// The name held by more than one game.
        name: String,
        /// Index of the game that first claimed it.
        first: usize,
    },

    /// Two or more games share an `id`, which `cloneofid` resolves through.
    DuplicateId {
        /// The id held by more than one game.
        id: String,
        /// Index of the game that first claimed it.
        first: usize,
    },

    /// `cloneof` names a game that is not in the datafile.
    UnknownCloneOf {
        /// The unresolvable name.
        name: String,
    },

    /// `cloneofid` names an id that no game in the datafile has.
    UnknownCloneOfId {
        /// The unresolvable id.
        id: String,
    },

    /// `romof` names a game that is not in the datafile.
    UnknownRomOf {
        /// The unresolvable name.
        name: String,
    },

    /// `sampleof` names a game that is not in the datafile.
    UnknownSampleOf {
        /// The unresolvable name.
        name: String,
    },

    /// A game claims itself as its own parent.
    SelfParent,

    /// `cloneof` and `cloneofid` resolve to different games.
    ConflictingParents {
        /// The game `cloneof` resolves to.
        by_name: String,
        /// The game `cloneofid` resolves to.
        by_id: String,
    },

    /// A `merge` attribute names a file the parent set does not contain.
    UnknownMerge {
        /// The file carrying the `merge`.
        file: String,
        /// The name it merges into.
        merge: String,
        /// The parent set searched.
        parent: String,
    },

    /// A `merge` attribute appears on a game with no parent to merge into.
    MergeWithoutParent {
        /// The file carrying the `merge`.
        file: String,
        /// The name it claims to merge into.
        merge: String,
    },

    /// The clone graph contains a cycle.
    CloneCycle {
        /// The games in the cycle, in link order, starting and ending at the
        /// same game.
        path: Vec<String>,
    },
}

impl fmt::Display for IssueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateName { name, first } => {
                write!(f, "name {name:?} is already used by game #{first}")
            }
            Self::DuplicateId { id, first } => {
                write!(f, "id {id:?} is already used by game #{first}")
            }
            Self::UnknownCloneOf { name } => {
                write!(f, "cloneof names an unknown game {name:?}")
            }
            Self::UnknownCloneOfId { id } => {
                write!(f, "cloneofid names an unknown id {id:?}")
            }
            Self::UnknownRomOf { name } => write!(f, "romof names an unknown game {name:?}"),
            Self::UnknownSampleOf { name } => {
                write!(f, "sampleof names an unknown game {name:?}")
            }
            Self::SelfParent => f.write_str("is its own parent"),
            Self::ConflictingParents { by_name, by_id } => write!(
                f,
                "cloneof resolves to {by_name:?} but cloneofid resolves to {by_id:?}"
            ),
            Self::UnknownMerge {
                file,
                merge,
                parent,
            } => write!(
                f,
                "{file:?} merges into {merge:?}, which parent {parent:?} does not contain"
            ),
            Self::MergeWithoutParent { file, merge } => {
                write!(
                    f,
                    "{file:?} merges into {merge:?} but the game has no parent"
                )
            }
            Self::CloneCycle { path } => {
                write!(f, "clone cycle: {}", path.join(" -> "))
            }
        }
    }
}

impl Datafile {
    /// Checks referential integrity, returning every problem found.
    ///
    /// An empty result means every `cloneof`, `cloneofid`, `romof`, `sampleof`
    /// and `merge` resolves, that names and ids are unique, and that the clone
    /// graph is acyclic.
    ///
    /// This never fails a datafile: parsing stays permissive, and the caller
    /// decides which findings matter. See the [module docs](self) for what is
    /// deliberately *not* checked.
    #[must_use]
    pub fn validate(&self) -> Vec<Issue> {
        let mut issues = Vec::new();

        let by_name = self.build_name_map(&mut issues);
        let by_id = self.build_id_map(&mut issues);

        for (index, game) in self.games.iter().enumerate() {
            self.check_parents(index, game, &by_name, &by_id, &mut issues);
            self.check_merges(game, &by_name, index, &mut issues);
        }

        self.check_cycles(&by_name, &by_id, &mut issues);

        issues
    }

    /// Maps game name to index, reporting collisions.
    fn build_name_map<'a>(&'a self, issues: &mut Vec<Issue>) -> HashMap<&'a str, usize> {
        let mut map = HashMap::with_capacity(self.games.len());
        for (index, game) in self.games.iter().enumerate() {
            if let Some(&first) = map.get(game.name.as_str()) {
                issues.push(Issue {
                    game: index,
                    game_name: game.name.clone(),
                    kind: IssueKind::DuplicateName {
                        name: game.name.clone(),
                        first,
                    },
                });
            } else {
                map.insert(game.name.as_str(), index);
            }
        }
        map
    }

    /// Maps game id to index, reporting collisions.
    fn build_id_map<'a>(&'a self, issues: &mut Vec<Issue>) -> HashMap<&'a str, usize> {
        let mut map = HashMap::new();
        for (index, game) in self.games.iter().enumerate() {
            let Some(id) = game.id.as_deref() else {
                continue;
            };
            if let Some(&first) = map.get(id) {
                issues.push(Issue {
                    game: index,
                    game_name: game.name.clone(),
                    kind: IssueKind::DuplicateId {
                        id: id.to_string(),
                        first,
                    },
                });
            } else {
                map.insert(id, index);
            }
        }
        map
    }

    fn check_parents(
        &self,
        index: usize,
        game: &Game,
        by_name: &HashMap<&str, usize>,
        by_id: &HashMap<&str, usize>,
        issues: &mut Vec<Issue>,
    ) {
        let issue = |kind| Issue {
            game: index,
            game_name: game.name.clone(),
            kind,
        };

        let mut resolved_by_name = None;
        if let Some(name) = &game.clone_of {
            match by_name.get(name.as_str()) {
                Some(&parent) => resolved_by_name = Some(parent),
                None => issues.push(issue(IssueKind::UnknownCloneOf { name: name.clone() })),
            }
        }

        let mut resolved_by_id = None;
        if let Some(id) = &game.clone_of_id {
            match by_id.get(id.as_str()) {
                Some(&parent) => resolved_by_id = Some(parent),
                None => issues.push(issue(IssueKind::UnknownCloneOfId { id: id.clone() })),
            }
        }

        // A game that is its own parent, by either route.
        if resolved_by_name == Some(index) || resolved_by_id == Some(index) {
            issues.push(issue(IssueKind::SelfParent));
        } else if let (Some(a), Some(b)) = (resolved_by_name, resolved_by_id) {
            // Both routes present and pointing at different games.
            if a != b {
                issues.push(issue(IssueKind::ConflictingParents {
                    by_name: self.games[a].name.clone(),
                    by_id: self.games[b].name.clone(),
                }));
            }
        }

        for (reference, kind) in [(&game.rom_of, 0u8), (&game.sample_of, 1)] {
            let Some(name) = reference else { continue };
            if by_name.contains_key(name.as_str()) {
                continue;
            }
            issues.push(issue(if kind == 0 {
                IssueKind::UnknownRomOf { name: name.clone() }
            } else {
                IssueKind::UnknownSampleOf { name: name.clone() }
            }));
        }
    }

    /// Checks that every `merge` names a file the parent actually has.
    ///
    /// ROM merging follows `romof` when present, falling back to `cloneof`,
    /// which is how the Logiqx DTD describes the relationship.
    fn check_merges(
        &self,
        game: &Game,
        by_name: &HashMap<&str, usize>,
        index: usize,
        issues: &mut Vec<Issue>,
    ) {
        let merges = game
            .roms
            .iter()
            .filter_map(|r| r.merge.as_ref().map(|m| (&r.name, m)))
            .chain(
                game.disks
                    .iter()
                    .filter_map(|d| d.merge.as_ref().map(|m| (&d.name, m))),
            );

        let parent = game
            .rom_of
            .as_deref()
            .or(game.clone_of.as_deref())
            .and_then(|name| by_name.get(name).map(|&i| &self.games[i]));

        for (file, merge) in merges {
            let issue = |kind| Issue {
                game: index,
                game_name: game.name.clone(),
                kind,
            };

            let Some(parent) = parent else {
                issues.push(issue(IssueKind::MergeWithoutParent {
                    file: file.clone(),
                    merge: merge.clone(),
                }));
                continue;
            };

            let present = parent.roms.iter().any(|r| &r.name == merge)
                || parent.disks.iter().any(|d| &d.name == merge);
            if !present {
                issues.push(issue(IssueKind::UnknownMerge {
                    file: file.clone(),
                    merge: merge.clone(),
                    parent: parent.name.clone(),
                }));
            }
        }
    }

    /// Reports each cycle in the clone graph once.
    fn check_cycles(
        &self,
        by_name: &HashMap<&str, usize>,
        by_id: &HashMap<&str, usize>,
        issues: &mut Vec<Issue>,
    ) {
        let parent_of = |index: usize| -> Option<usize> {
            let game = &self.games[index];
            game.clone_of
                .as_deref()
                .and_then(|n| by_name.get(n).copied())
                .or_else(|| {
                    game.clone_of_id
                        .as_deref()
                        .and_then(|i| by_id.get(i).copied())
                })
        };

        for start in 0..self.games.len() {
            let mut path = vec![start];
            let mut current = start;

            // Walk up the chain. A cycle is only reported from its lowest
            // index, so a three-game loop yields one finding rather than three.
            while let Some(next) = parent_of(current) {
                if next == start {
                    // A one-game loop is already reported as `SelfParent`,
                    // which says the same thing more directly.
                    if path.len() > 1 && path.iter().all(|&i| i >= start) {
                        let mut names: Vec<String> =
                            path.iter().map(|&i| self.games[i].name.clone()).collect();
                        names.push(self.games[start].name.clone());
                        issues.push(Issue {
                            game: start,
                            game_name: self.games[start].name.clone(),
                            kind: IssueKind::CloneCycle { path: names },
                        });
                    }
                    break;
                }
                if path.contains(&next) {
                    // A cycle that does not include `start`; it will be
                    // reported when the walk begins inside it.
                    break;
                }
                path.push(next);
                current = next;
            }
        }
    }
}
