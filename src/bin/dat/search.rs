//! Narrowing a datafile down to the games a query asks for.

use datary::{Game, Md5, Sha1, Sha256};
use std::fmt;
use unicode_normalization::UnicodeNormalization;

/// What to look for. The command line accepts at most one.
pub enum Query {
    /// Games with a ROM or disk of this SHA-1.
    Sha1(Sha1),
    /// Games with a ROM of this SHA-256.
    Sha256(Sha256),
    /// Games with a ROM or disk of this MD5.
    Md5(Md5),
    /// Games whose name or description loosely matches.
    Title(Title),
}

impl Query {
    /// Returns `true` if `game` is one the query asks for.
    pub fn matches(&self, game: &Game) -> bool {
        match self {
            // Disks carry checksums too, so a CHD is found the same way a ROM is.
            Self::Sha1(sha1) => {
                game.roms.iter().any(|r| r.sha1.as_ref() == Some(sha1))
                    || game.disks.iter().any(|d| d.sha1.as_ref() == Some(sha1))
            }
            // ...but no SHA-256; that is a No-Intro ROM attribute only.
            Self::Sha256(sha256) => game.roms.iter().any(|r| r.sha256.as_ref() == Some(sha256)),
            Self::Md5(md5) => {
                game.roms.iter().any(|r| r.md5.as_ref() == Some(md5))
                    || game.disks.iter().any(|d| d.md5.as_ref() == Some(md5))
            }
            Self::Title(title) => title.matches(&game.name) || title.matches(&game.description),
        }
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sha1(sha1) => write!(f, "sha1 {sha1}"),
            Self::Sha256(sha256) => write!(f, "sha256 {sha256}"),
            Self::Md5(md5) => write!(f, "md5 {md5}"),
            Self::Title(title) => write!(f, "title {:?}", title.query),
        }
    }
}

/// A loose title query: words that must all appear, in order, with anything at
/// all between them.
///
/// `A C` therefore matches `A: b C`, and `mario land` matches
/// `Super Mario Land 2 - 6 Golden Coins (USA, Europe)`. A word matches anywhere
/// inside a word of the title, so `mar` matches `Mario` too. Both sides are
/// compared [`fold`]ed, so case and accents do not matter either way round.
#[derive(Debug, Clone)]
pub struct Title {
    /// The query as typed, for messages.
    query: String,
    /// The query folded and split on whitespace.
    words: Vec<String>,
}

impl Title {
    /// Parses a query, rejecting one with no words in it — it would match
    /// every game, which is what leaving `--title` out already does.
    pub fn parse(query: &str) -> Result<Self, String> {
        let words: Vec<String> = fold(query).split_whitespace().map(str::to_owned).collect();
        if words.is_empty() {
            return Err("expected at least one word".to_owned());
        }
        Ok(Self {
            query: query.to_owned(),
            words,
        })
    }

    /// Returns `true` if every word appears in `candidate`, in order.
    pub fn matches(&self, candidate: &str) -> bool {
        let candidate = fold(candidate);
        let mut rest = candidate.as_str();
        // Taking the leftmost occurrence of each word leaves the most room for
        // the ones after it, so a greedy scan never misses a match.
        self.words
            .iter()
            .all(|word| match rest.find(word.as_str()) {
                Some(at) => {
                    rest = &rest[at + word.len()..];
                    true
                }
                None => false,
            })
    }

    /// The name or description closest to this query, for a "did you mean"
    /// hint when nothing matched.
    ///
    /// Closeness is Levenshtein distance, but measured per word against the
    /// closest *substring* of each candidate rather than against the whole of
    /// it. Plain Levenshtein is dominated by length: `zelda` is 21 edits from
    /// `Legend of Zelda, The (USA)` but only 4 from `Zoda (Japan)`. Scoring
    /// each word separately also forgives words given out of order. Ties go to
    /// the candidate closest to the query as a whole, then to the earliest.
    ///
    /// Returns [`None`] when even the best candidate needs edits to more than
    /// half the query's characters, since that is a guess rather than a hint.
    pub fn suggest<'a>(&self, games: impl IntoIterator<Item = &'a Game>) -> Option<&'a str> {
        let words: Vec<Vec<char>> = self.words.iter().map(|w| w.chars().collect()).collect();
        let whole: Vec<char> = self.words.join(" ").chars().collect();
        let letters: usize = words.iter().map(Vec::len).sum();

        let (score, _, best) = games
            .into_iter()
            .flat_map(|game| [game.name.as_str(), game.description.as_str()])
            .filter(|candidate| !candidate.is_empty())
            .map(|candidate| {
                let text: Vec<char> = fold(candidate).chars().collect();
                let score: usize = words.iter().map(|w| edit_distance(w, &text, true)).sum();
                (score, edit_distance(&whole, &text, false), candidate)
            })
            // `min_by_key` keeps the first of equal keys, so earlier games win ties.
            .min_by_key(|&(score, overall, _)| (score, overall))?;

        (score * 2 <= letters).then_some(best)
    }
}

/// Reduces text to the form titles are compared in, so that `Pokémon`,
/// `POKEMON`, `Poke\u{301}mon` (an `e` followed by a combining acute) and the
/// full-width `Ｐｏｋéｍｏｎ` all become `pokemon`.
///
/// Compatibility decomposition (NFKD) first splits accented letters into a
/// base letter and combining marks, and maps ligatures and full-width forms to
/// plain letters. The combining marks are then dropped — but only those in the
/// Combining Diacritical Marks block, which holds the accents on Latin, Greek
/// and Cyrillic letters. Dropping every combining mark would also erase the
/// voicing marks that tell kana apart (ポ, ボ and ホ are three syllables),
/// which are letters in their own right rather than accents.
///
/// A handful of Latin letters have no decomposition to fall back on, and are
/// spelled out as their usual ASCII stand-ins instead: `ø` as `o`, `ß` as
/// `ss`, `æ` as `ae`, and so on.
pub fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    let letters = text
        .nfkd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        // After decomposing, as that can produce capitals: `Ⅻ` is `XII`.
        .flat_map(char::to_lowercase);
    for c in letters {
        match c {
            'ß' => folded.push_str("ss"),
            'æ' => folded.push_str("ae"),
            'œ' => folded.push_str("oe"),
            'þ' => folded.push_str("th"),
            'ø' => folded.push('o'),
            'đ' | 'ð' => folded.push('d'),
            'ł' => folded.push('l'),
            'ı' => folded.push('i'),
            c => folded.push(c),
        }
    }
    folded
}

/// Levenshtein distance from `a` to `b`, or with `within` set, from `a` to the
/// closest substring of `b`.
///
/// The substring form is the classic approximate-matching variant: skipping
/// any prefix of `b` is free (the first row starts at zero), and so is any
/// suffix (the answer is the minimum over the last row).
fn edit_distance(a: &[char], b: &[char], within: bool) -> usize {
    let mut row: Vec<usize> = if within {
        vec![0; b.len() + 1]
    } else {
        (0..=b.len()).collect()
    };

    for (i, &ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let substitute = diagonal + usize::from(ca != cb);
            diagonal = row[j + 1];
            row[j + 1] = substitute.min(row[j + 1] + 1).min(row[j] + 1);
        }
    }

    if within {
        row.into_iter().min().unwrap_or_default()
    } else {
        row[b.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title(query: &str) -> Title {
        Title::parse(query).unwrap()
    }

    fn game(name: &str) -> Game {
        Game {
            name: name.to_owned(),
            description: name.to_owned(),
            ..Game::default()
        }
    }

    fn distance(a: &str, b: &str, within: bool) -> usize {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        edit_distance(&a, &b, within)
    }

    #[test]
    fn words_match_in_order_with_anything_between() {
        assert!(title("A C").matches("A: b C"));
        assert!(title("mario land").matches("Super Mario Land 2 (USA)"));
        assert!(!title("C A").matches("A: b C"), "order matters");
    }

    #[test]
    fn matching_ignores_case_and_word_boundaries() {
        assert!(title("TETRIS").matches("3-D Tetris (USA)"));
        assert!(title("mar").matches("Mario"));
    }

    #[test]
    fn folding_drops_accents_case_and_compatibility_forms() {
        assert_eq!(fold("Pokémon"), "pokemon");
        assert_eq!(fold("Poke\u{301}mon"), "pokemon", "a decomposed é");
        assert_eq!(fold("ＰＯＫＥＭＯＮ"), "pokemon", "full-width letters");
        assert_eq!(fold("ﬁnal Ⅻ"), "final xii", "a ligature and a numeral");
        assert_eq!(fold("İstanbul Ångström Ñandú"), "istanbul angstrom nandu");
    }

    #[test]
    fn folding_spells_out_letters_that_do_not_decompose() {
        assert_eq!(fold("Straße"), "strasse");
        assert_eq!(
            fold("Æon Œuvre Ørsted Łódź Þór"),
            "aeon oeuvre orsted lodz thor"
        );
    }

    #[test]
    fn folding_keeps_kana_voicing_marks() {
        // ボ, ポ and ホ are different syllables, not one letter with accents.
        assert_eq!(fold("ボ"), "ホ\u{3099}");
        assert_eq!(fold("ポ"), "ホ\u{309a}");
        assert_ne!(fold("ボンバーマン"), fold("ホンハーマン"));
        assert!(title("ボンバーマン").matches("ボンバーマン ジャパン"));
    }

    #[test]
    fn accents_match_either_way_round() {
        assert!(title("pokemon").matches("Pokémon Pinball"));
        assert!(title("pokémon").matches("Pokemon Party Mini (USA)"));
        assert!(
            title("pokémon").matches("Poke\u{301}mon"),
            "composed against decomposed"
        );
        assert!(title("strasse").matches("Straße"));
    }

    #[test]
    fn a_word_is_not_counted_twice() {
        assert!(!title("aa aa").matches("aaa"));
        assert!(title("aa aa").matches("aaaa"));
    }

    #[test]
    fn an_empty_query_is_rejected() {
        assert!(Title::parse("").is_err());
        assert!(Title::parse(" \t ").is_err());
    }

    #[test]
    fn edit_distance_is_levenshtein() {
        assert_eq!(distance("kitten", "sitting", false), 3);
        assert_eq!(distance("", "abc", false), 3);
        assert_eq!(distance("abc", "", false), 3);
        assert_eq!(distance("same", "same", false), 0);
    }

    #[test]
    fn substring_distance_ignores_the_surroundings() {
        assert_eq!(distance("zelda", "legend of zelda, the (usa)", true), 0);
        assert_eq!(distance("mairo", "super mario land", true), 2);
        assert_eq!(distance("pacman", "pac-man (usa)", true), 1);
        assert_eq!(distance("abc", "", true), 3);
    }

    #[test]
    fn suggestions_prefer_the_title_containing_a_near_miss() {
        let games = [game("Zoda (Japan)"), game("Legend of Zelda, The (USA)")];
        assert_eq!(
            title("zeldo").suggest(&games),
            Some("Legend of Zelda, The (USA)")
        );
    }

    #[test]
    fn suggestions_forgive_word_order() {
        let games = [game("Tetris (USA)"), game("Super Mario Land (World)")];
        assert_eq!(
            title("land mario").suggest(&games),
            Some("Super Mario Land (World)")
        );
    }

    #[test]
    fn ties_go_to_the_closest_whole_title() {
        let games = [
            game("Super Mario Land 2 - 6 Golden Coins (USA, Europe)"),
            game("Super Mario Land (World)"),
        ];
        assert_eq!(
            title("super mairo land").suggest(&games),
            Some("Super Mario Land (World)")
        );
    }

    #[test]
    fn suggestions_ignore_accents() {
        let games = [game("Tetris (USA)"), game("Pokémon Pinball (Europe)")];
        assert_eq!(
            title("pokemn").suggest(&games),
            Some("Pokémon Pinball (Europe)")
        );
    }

    #[test]
    fn nothing_is_suggested_when_nothing_is_close() {
        let games = [game("Tetris (USA)")];
        assert_eq!(title("xyzzy").suggest(&games), None);
        assert_eq!(title("anything").suggest(&[]), None);
    }
}
