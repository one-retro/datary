//! Checking files on disk against a datafile.
//!
//! [`FileHashes`] computes every checksum a datafile can carry in a single pass
//! over the data, and [`Rom::verify`] compares an entry against it, reporting
//! *which* checksums disagreed rather than a bare boolean.
//!
//! ```no_run
//! use datary::verify::FileHashes;
//!
//! let dat = datary::read_file("Nintendo - Virtual Boy.dat")?.indexed();
//! let hashes = FileHashes::of_file("3-D Tetris (USA).vb")?;
//!
//! match dat.find(&hashes) {
//!     Some((game, _rom)) => println!("recognised: {}", game.name),
//!     None => println!("not in the datafile"),
//! }
//! # Ok::<(), datary::Error>(())
//! ```

use crate::dat::Rom;
use crate::hash::{Crc32, Md5, Sha1, Sha256};
use std::fmt;
use std::io::Read;
use std::path::Path;

#[cfg(feature = "index")]
use crate::dat::Game;
#[cfg(feature = "index")]
use crate::index::IndexedDatafile;

/// Every checksum a datafile can record, computed over one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHashes {
    /// Size in bytes.
    pub size: u64,
    /// CRC32 of the contents.
    pub crc: Crc32,
    /// MD5 of the contents.
    pub md5: Md5,
    /// SHA-1 of the contents.
    pub sha1: Sha1,
    /// SHA-256 of the contents.
    pub sha256: Sha256,
}

impl FileHashes {
    /// Computes every checksum over a byte slice.
    #[must_use]
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self {
            size: bytes.len() as u64,
            crc: Crc32::compute(bytes),
            md5: Md5::compute(bytes),
            sha1: Sha1::compute(bytes),
            sha256: Sha256::compute(bytes),
        }
    }

    /// Computes every checksum in a single streaming pass over `reader`.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the reader.
    pub fn of_reader(mut reader: impl Read) -> std::io::Result<Self> {
        use digest::Digest as _;

        let mut crc = crc32fast::Hasher::new();
        let mut md5 = md5::Md5::new();
        let mut sha1 = sha1::Sha1::new();
        let mut sha256 = sha2::Sha256::new();
        let mut size = 0u64;

        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let chunk = &buf[..n];
            crc.update(chunk);
            md5.update(chunk);
            sha1.update(chunk);
            sha256.update(chunk);
            size += n as u64;
        }

        Ok(Self {
            size,
            crc: Crc32(crc.finalize()),
            md5: md5.finalize().into(),
            sha1: sha1.finalize().into(),
            sha256: sha256.finalize().into(),
        })
    }

    /// Computes every checksum over a file on disk.
    ///
    /// # Errors
    ///
    /// Returns any error produced while opening or reading the file.
    pub fn of_file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = std::fs::File::open(path.as_ref())?;
        Self::of_reader(std::io::BufReader::new(file))
    }
}

/// One way in which a file failed to match a datafile entry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mismatch {
    /// The file size differed.
    Size {
        /// Size recorded in the datafile.
        expected: u64,
        /// Size of the file on disk.
        actual: u64,
    },
    /// The CRC32 differed.
    Crc32 {
        /// Checksum recorded in the datafile.
        expected: Crc32,
        /// Checksum of the file on disk.
        actual: Crc32,
    },
    /// The MD5 differed.
    Md5 {
        /// Checksum recorded in the datafile.
        expected: Md5,
        /// Checksum of the file on disk.
        actual: Md5,
    },
    /// The SHA-1 differed.
    Sha1 {
        /// Checksum recorded in the datafile.
        expected: Sha1,
        /// Checksum of the file on disk.
        actual: Sha1,
    },
    /// The SHA-256 differed.
    Sha256 {
        /// Checksum recorded in the datafile.
        expected: Sha256,
        /// Checksum of the file on disk.
        actual: Sha256,
    },
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Size { expected, actual } => {
                write!(f, "size mismatch: expected {expected}, got {actual}")
            }
            Self::Crc32 { expected, actual } => {
                write!(f, "crc mismatch: expected {expected}, got {actual}")
            }
            Self::Md5 { expected, actual } => {
                write!(f, "md5 mismatch: expected {expected}, got {actual}")
            }
            Self::Sha1 { expected, actual } => {
                write!(f, "sha1 mismatch: expected {expected}, got {actual}")
            }
            Self::Sha256 { expected, actual } => {
                write!(f, "sha256 mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

impl Rom {
    /// Checks `hashes` against every checksum this entry records.
    ///
    /// Checksums absent from the entry are not checked. An entry with no
    /// checksums at all still verifies its size.
    ///
    /// # Errors
    ///
    /// Returns the list of checksums that disagreed, in size/crc/md5/sha1/sha256
    /// order. An empty list is never returned as an error.
    #[cfg_attr(docsrs, doc(cfg(feature = "verify")))]
    pub fn verify(&self, hashes: &FileHashes) -> Result<(), Vec<Mismatch>> {
        let mut problems = Vec::new();

        if self.size != hashes.size {
            problems.push(Mismatch::Size {
                expected: self.size,
                actual: hashes.size,
            });
        }
        if let Some(expected) = self.crc {
            if expected != hashes.crc {
                problems.push(Mismatch::Crc32 {
                    expected,
                    actual: hashes.crc,
                });
            }
        }
        if let Some(expected) = self.md5 {
            if expected != hashes.md5 {
                problems.push(Mismatch::Md5 {
                    expected,
                    actual: hashes.md5,
                });
            }
        }
        if let Some(expected) = self.sha1 {
            if expected != hashes.sha1 {
                problems.push(Mismatch::Sha1 {
                    expected,
                    actual: hashes.sha1,
                });
            }
        }
        if let Some(expected) = self.sha256 {
            if expected != hashes.sha256 {
                problems.push(Mismatch::Sha256 {
                    expected,
                    actual: hashes.sha256,
                });
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }

    /// Returns `true` if `hashes` matches every checksum this entry records.
    #[must_use]
    #[cfg_attr(docsrs, doc(cfg(feature = "verify")))]
    pub fn matches(&self, hashes: &FileHashes) -> bool {
        self.verify(hashes).is_ok()
    }
}

#[cfg(feature = "index")]
#[cfg_attr(docsrs, doc(cfg(all(feature = "verify", feature = "index"))))]
impl IndexedDatafile {
    /// Finds the entry matching a hashed file.
    ///
    /// Looks up by the strongest checksum available — SHA-256, then SHA-1, MD5
    /// and finally CRC32 — and returns the first candidate that also agrees on
    /// every other checksum the entry records.
    #[must_use]
    pub fn find(&self, hashes: &FileHashes) -> Option<(&Game, &Rom)> {
        let candidates = {
            let by_sha256 = self.index().by_sha256(&hashes.sha256);
            let by_sha1 = self.index().by_sha1(&hashes.sha1);
            let by_md5 = self.index().by_md5(&hashes.md5);
            if !by_sha256.is_empty() {
                by_sha256
            } else if !by_sha1.is_empty() {
                by_sha1
            } else if !by_md5.is_empty() {
                by_md5
            } else {
                self.index().by_crc(hashes.crc)
            }
        };

        candidates
            .iter()
            .map(|&r| self.resolve(r))
            .find(|(_, rom)| rom.matches(hashes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::Status;

    fn rom_for(data: &[u8]) -> Rom {
        let h = FileHashes::of_bytes(data);
        Rom {
            name: "test.rom".into(),
            size: h.size,
            crc: Some(h.crc),
            md5: Some(h.md5),
            sha1: Some(h.sha1),
            sha256: Some(h.sha256),
            ..Rom::default()
        }
    }

    #[test]
    fn matching_file_verifies() {
        let rom = rom_for(b"hello world");
        assert!(rom.matches(&FileHashes::of_bytes(b"hello world")));
        assert!(rom.verify(&FileHashes::of_bytes(b"hello world")).is_ok());
    }

    #[test]
    fn every_differing_checksum_is_reported() {
        let rom = rom_for(b"hello world");
        // Same length, so size agrees but all four checksums differ.
        let problems = rom
            .verify(&FileHashes::of_bytes(b"world hello"))
            .unwrap_err();
        assert_eq!(problems.len(), 4);
        assert!(matches!(problems[0], Mismatch::Crc32 { .. }));
        assert!(matches!(problems[3], Mismatch::Sha256 { .. }));
    }

    #[test]
    fn size_mismatch_is_reported_first() {
        let rom = rom_for(b"hello world");
        let problems = rom.verify(&FileHashes::of_bytes(b"short")).unwrap_err();
        assert_eq!(
            problems[0],
            Mismatch::Size {
                expected: 11,
                actual: 5
            }
        );
    }

    #[test]
    fn absent_checksums_are_not_checked() {
        // A nodump entry records nothing but a size of zero.
        let rom = Rom {
            name: "missing.rom".into(),
            size: 0,
            status: Some(Status::NoDump),
            ..Rom::default()
        };
        assert!(rom.matches(&FileHashes::of_bytes(b"")));
        assert!(!rom.matches(&FileHashes::of_bytes(b"x")));
    }

    #[test]
    fn streaming_and_slice_hashing_agree() {
        let data = vec![3u8; 300_000];
        assert_eq!(
            FileHashes::of_bytes(&data),
            FileHashes::of_reader(&data[..]).unwrap()
        );
    }

    #[test]
    fn mismatch_displays_hex() {
        let m = Mismatch::Crc32 {
            expected: Crc32(0x1234_5678),
            actual: Crc32(0),
        };
        assert_eq!(
            m.to_string(),
            "crc mismatch: expected 12345678, got 00000000"
        );
    }
}
