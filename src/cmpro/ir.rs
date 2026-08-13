//! The block tree a ClrMamePro datafile parses into.
//!
//! This sits between the grammar and [`Datafile`](crate::Datafile). Keeping it
//! separate means the parser stays a pure syntax concern and all the knowledge
//! of *which keys mean what* lives in one place, which matters for a format
//! with no specification and several dialects.

/// A `name ( ... )` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block<'a> {
    /// The block's name, such as `clrmamepro`, `game` or `rom`.
    pub name: &'a str,
    /// Its entries, in source order.
    pub entries: Vec<Entry<'a>>,
}

/// One item inside a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry<'a> {
    /// `key value` — a bare or quoted scalar.
    Field(&'a str, &'a str),
    /// `key ( ... )` — a nested block.
    Block(Block<'a>),
}

impl<'a> Block<'a> {
    /// The first scalar value for `key`.
    #[must_use]
    pub fn field(&self, key: &str) -> Option<&'a str> {
        self.entries.iter().find_map(|e| match e {
            Entry::Field(k, v) if *k == key => Some(*v),
            _ => None,
        })
    }

    /// The first scalar value for whichever of `keys` appears first.
    ///
    /// Used for the dialect splits: ClrMamePro writes `crc`, ckmame writes
    /// `crc32`; the dump status is `flags` here and `status` in XML.
    #[must_use]
    pub fn any_field(&self, keys: &[&str]) -> Option<&'a str> {
        keys.iter().find_map(|k| self.field(k))
    }

    /// Every nested block named `name`.
    pub fn blocks<'s>(&'s self, name: &'s str) -> impl Iterator<Item = &'s Block<'a>> + 's {
        self.entries.iter().filter_map(move |e| match e {
            Entry::Block(b) if b.name == name => Some(b),
            _ => None,
        })
    }
}
