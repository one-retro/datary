//! Enumerated attribute values.
//!
//! Every attribute that the Logiqx DTD or the No-Intro schema constrains to a
//! fixed set of tokens is modelled as a Rust enum, so invalid values are
//! rejected at parse time rather than surfacing later as a string comparison
//! that silently never matches.
//!
//! Parsing is lenient about spelling (`yes`, `Yes`, `true` and `1` all mean the
//! same thing); writing always emits the canonical lowercase token from the spec.

use serde::{Deserialize, Serialize};

/// A boolean attribute, spelled `yes` or `no` in XML.
///
/// Kept as a distinct type rather than a `bool` because XML writes it as a
/// token, not as `true`/`false`.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum YesNo {
    /// `no`
    #[default]
    #[serde(alias = "No", alias = "NO", alias = "false", alias = "0")]
    No,
    /// `yes`
    #[serde(alias = "Yes", alias = "YES", alias = "true", alias = "1")]
    Yes,
}

impl YesNo {
    /// Returns `true` for [`YesNo::Yes`].
    #[must_use]
    pub const fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}

impl From<bool> for YesNo {
    fn from(value: bool) -> Self {
        if value {
            Self::Yes
        } else {
            Self::No
        }
    }
}

impl From<YesNo> for bool {
    fn from(value: YesNo) -> Self {
        value.is_yes()
    }
}

/// Dump status of a ROM or disk, from the `status` attribute.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// The dump is known to be correct.
    #[default]
    Good,
    /// The dump is known to be corrupt.
    #[serde(alias = "bad")]
    BadDump,
    /// No correct dump is known to exist.
    #[serde(alias = "no")]
    NoDump,
    /// The dump has been verified against another source.
    Verified,
}

impl Status {
    /// Returns `true` if this status describes a usable dump.
    #[must_use]
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Good | Self::Verified)
    }
}

/// Merge behaviour requested by the datafile, from `clrmamepro/@forcemerging`.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ForceMerging {
    /// Do not merge clones into parents.
    None,
    /// Merge clones into their parent, keeping distinct files. The DTD default.
    #[default]
    Split,
    /// Merge everything into the parent set.
    Full,
}

/// How missing dumps should be treated, from `clrmamepro/@forcenodump`.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ForceNoDump {
    /// Treat `nodump` entries as obsolete. The DTD default.
    #[default]
    Obsolete,
    /// `nodump` entries are required to be present.
    Required,
    /// Ignore `nodump` entries entirely.
    Ignore,
}

/// Packing behaviour requested by the datafile, from `clrmamepro/@forcepacking`.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ForcePacking {
    /// Sets should be stored zipped. The DTD default.
    #[default]
    Zip,
    /// Sets should be stored unzipped.
    Unzip,
}

/// ROM set layout, from `romcenter/@rommode` and `romcenter/@biosmode`.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum RomMode {
    /// Clones are stored inside their parent set.
    Merged,
    /// Clones are stored separately from their parent. The DTD default.
    #[default]
    Split,
    /// Every set is complete and self-contained.
    Unmerged,
}

/// Sample set layout, from `romcenter/@samplemode`.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum SampleMode {
    /// Samples are shared between sets. The DTD default.
    #[default]
    Merged,
    /// Every set carries its own samples.
    Unmerged,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips a value through a single XML attribute.
    fn attr<T: Serialize + for<'de> Deserialize<'de>>(xml: &str) -> T {
        #[derive(Serialize, Deserialize)]
        struct Wrap<T> {
            #[serde(rename = "@v")]
            v: T,
        }
        let w: Wrap<T> = quick_xml::de::from_str(&format!("<w v=\"{xml}\"/>")).unwrap();
        w.v
    }

    #[test]
    fn yes_no_accepts_spelling_variants() {
        for s in ["yes", "Yes", "YES", "true", "1"] {
            assert_eq!(attr::<YesNo>(s), YesNo::Yes, "parsing {s:?}");
        }
        for s in ["no", "No", "NO", "false", "0"] {
            assert_eq!(attr::<YesNo>(s), YesNo::No, "parsing {s:?}");
        }
    }

    #[test]
    fn yes_no_converts_to_bool() {
        assert!(YesNo::Yes.is_yes());
        assert!(!YesNo::No.is_yes());
        assert_eq!(YesNo::from(true), YesNo::Yes);
        assert!(bool::from(YesNo::Yes));
    }

    #[test]
    fn status_parses_dtd_tokens() {
        assert_eq!(attr::<Status>("good"), Status::Good);
        assert_eq!(attr::<Status>("baddump"), Status::BadDump);
        assert_eq!(attr::<Status>("nodump"), Status::NoDump);
        assert_eq!(attr::<Status>("verified"), Status::Verified);
        assert!(Status::Good.is_usable());
        assert!(Status::Verified.is_usable());
        assert!(!Status::BadDump.is_usable());
        assert!(!Status::NoDump.is_usable());
    }

    #[test]
    fn defaults_match_the_dtd() {
        assert_eq!(Status::default(), Status::Good);
        assert_eq!(ForceMerging::default(), ForceMerging::Split);
        assert_eq!(ForceNoDump::default(), ForceNoDump::Obsolete);
        assert_eq!(ForcePacking::default(), ForcePacking::Zip);
        assert_eq!(RomMode::default(), RomMode::Split);
        assert_eq!(SampleMode::default(), SampleMode::Merged);
        assert_eq!(YesNo::default(), YesNo::No);
    }

    #[test]
    fn mode_enums_parse() {
        assert_eq!(attr::<ForceMerging>("full"), ForceMerging::Full);
        assert_eq!(attr::<ForceNoDump>("required"), ForceNoDump::Required);
        assert_eq!(attr::<ForcePacking>("unzip"), ForcePacking::Unzip);
        assert_eq!(attr::<RomMode>("unmerged"), RomMode::Unmerged);
        assert_eq!(attr::<SampleMode>("unmerged"), SampleMode::Unmerged);
    }

    #[test]
    fn unknown_tokens_are_rejected() {
        #[derive(Deserialize)]
        struct Wrap<T> {
            #[serde(rename = "@v")]
            #[allow(dead_code)]
            v: T,
        }
        assert!(quick_xml::de::from_str::<Wrap<Status>>("<w v=\"sideways\"/>").is_err());
        assert!(quick_xml::de::from_str::<Wrap<YesNo>>("<w v=\"maybe\"/>").is_err());
        assert!(quick_xml::de::from_str::<Wrap<ForcePacking>>("<w v=\"tar\"/>").is_err());
    }
}
