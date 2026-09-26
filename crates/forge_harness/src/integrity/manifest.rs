//! Hashing protected files before and after a run, and putting them back.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Recorded state of one protected file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileState {
    /// SHA-256 of the file's bytes, lowercase hex.
    pub sha256: String,
    /// Size in bytes, recorded so an obviously different file is visible in the
    /// evidence without comparing hashes by eye.
    pub bytes: u64,
}

/// Hashes of every protected file, captured before the run starts.
///
/// The snapshot directory holds copies of the file contents, which is what
/// makes restoration possible; hashes alone can prove a violation but cannot
/// undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Repository root the paths are relative to.
    pub root: PathBuf,
    /// Protected files, keyed by path relative to `root`.
    pub files: BTreeMap<String, FileState>,
    /// Where file copies were written, when snapshotting succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_dir: Option<PathBuf>,
}

/// How a protected file was altered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationKind {
    /// Contents differ from the captured hash.
    Modified,
    /// The file is gone.
    Deleted,
    /// A protected file exists that was not there before.
    Added,
}

/// One protected file that changed during the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    /// Path relative to the repository root.
    pub path: String,
    /// What happened to it.
    pub kind: ViolationKind,
    /// Whether the original content was put back.
    pub restored: bool,
}

/// Outcome of comparing the repository against the manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityReport {
    /// Files that changed. Empty means the run left every test alone.
    pub violations: Vec<Violation>,
    /// Number of protected files checked.
    pub checked: usize,
}

impl IntegrityReport {
    /// Whether every protected file is exactly as it was.
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

impl Manifest {
    /// Hashes every protected file and copies it into `snapshot_dir`.
    ///
    /// Files that cannot be read are skipped with a warning rather than
    /// aborting: a partially captured manifest still protects everything it did
    /// capture, whereas failing here would leave the run with no guard at all.
    pub fn capture(root: &Path, protected_files: &[String], snapshot_dir: Option<&Path>) -> Self {
        let mut files = BTreeMap::new();

        for relative in protected_files {
            let absolute = root.join(relative);
            let Ok(bytes) = std::fs::read(&absolute) else {
                tracing::warn!(path = %relative, "Could not read protected file while capturing manifest");
                continue;
            };
            if let Some(dir) = snapshot_dir {
                let target = dir.join(relative);
                let copied = target
                    .parent()
                    .map(std::fs::create_dir_all)
                    .transpose()
                    .and_then(|_| std::fs::write(&target, &bytes));
                if let Err(error) = copied {
                    tracing::warn!(path = %relative, ?error, "Could not snapshot protected file");
                }
            }
            files.insert(
                relative.clone(),
                FileState { sha256: hash_bytes(&bytes), bytes: bytes.len() as u64 },
            );
        }

        Self {
            root: root.to_path_buf(),
            files,
            snapshot_dir: snapshot_dir.map(Path::to_path_buf),
        }
    }

    /// Compares the repository against the manifest.
    pub fn verify(&self, extra_protected: &[String]) -> IntegrityReport {
        let mut violations = Vec::new();

        for (relative, expected) in &self.files {
            let absolute = self.root.join(relative);
            match std::fs::read(&absolute) {
                Err(_) => violations.push(Violation {
                    path: relative.clone(),
                    kind: ViolationKind::Deleted,
                    restored: false,
                }),
                Ok(bytes) if hash_bytes(&bytes) != expected.sha256 => violations.push(Violation {
                    path: relative.clone(),
                    kind: ViolationKind::Modified,
                    restored: false,
                }),
                Ok(_) => {}
            }
        }

        // A test file that appeared during the run is also a violation: adding
        // a passing test beside a failing one is a way to make a suite look
        // green without fixing anything.
        for relative in extra_protected {
            if !self.files.contains_key(relative) {
                violations.push(Violation {
                    path: relative.clone(),
                    kind: ViolationKind::Added,
                    restored: false,
                });
            }
        }

        IntegrityReport { violations, checked: self.files.len() }
    }

    /// Puts modified and deleted files back, and removes added ones.
    ///
    /// Restoration keeps the submitted repository honest even if something got
    /// through the tool and shell guards. The violation stays in the report
    /// either way, so this hides nothing — it only prevents a tampered tree
    /// from being what the judges inspect (`DECISIONS.md` D-019).
    pub fn restore(&self, report: &IntegrityReport) -> IntegrityReport {
        let Some(snapshot_dir) = self.snapshot_dir.as_ref() else {
            tracing::warn!("No snapshot directory; protected files cannot be restored");
            return report.clone();
        };

        let violations = report
            .violations
            .iter()
            .map(|violation| {
                let target = self.root.join(&violation.path);
                let restored = match violation.kind {
                    ViolationKind::Modified | ViolationKind::Deleted => {
                        let source = snapshot_dir.join(&violation.path);
                        std::fs::read(&source)
                            .and_then(|bytes| {
                                if let Some(parent) = target.parent() {
                                    std::fs::create_dir_all(parent)?;
                                }
                                std::fs::write(&target, bytes)
                            })
                            .is_ok()
                    }
                    ViolationKind::Added => std::fs::remove_file(&target).is_ok(),
                };
                if !restored {
                    tracing::warn!(path = %violation.path, "Could not restore protected file");
                }
                Violation { restored, ..violation.clone() }
            })
            .collect();

        IntegrityReport { violations, checked: report.checked }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        snapshot: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let snapshot = dir.path().join("snapshot");
        std::fs::create_dir_all(root.join("tests")).unwrap();
        std::fs::create_dir_all(&snapshot).unwrap();
        std::fs::write(root.join("tests/test_math.py"), b"assert add(1, 2) == 3\n").unwrap();
        std::fs::write(root.join("src.py"), b"def add(a, b): return 0\n").unwrap();
        Fixture { _dir: dir, root, snapshot }
    }

    fn protected() -> Vec<String> {
        vec!["tests/test_math.py".to_string()]
    }

    #[test]
    fn test_an_untouched_repository_verifies_clean() {
        let fixture = fixture();
        let manifest = Manifest::capture(&fixture.root, &protected(), Some(&fixture.snapshot));

        let actual = manifest.verify(&protected());

        assert_eq!(actual, IntegrityReport { violations: vec![], checked: 1 });
        assert!(actual.is_clean());
    }

    #[test]
    fn test_a_modified_test_is_detected_and_restored() {
        let fixture = fixture();
        let manifest = Manifest::capture(&fixture.root, &protected(), Some(&fixture.snapshot));
        std::fs::write(fixture.root.join("tests/test_math.py"), b"assert True\n").unwrap();

        let detected = manifest.verify(&protected());
        let actual = manifest.restore(&detected);

        let expected = IntegrityReport {
            violations: vec![Violation {
                path: "tests/test_math.py".to_string(),
                kind: ViolationKind::Modified,
                restored: true,
            }],
            checked: 1,
        };
        assert_eq!(actual, expected);
        let contents = std::fs::read(fixture.root.join("tests/test_math.py")).unwrap();
        assert_eq!(contents, b"assert add(1, 2) == 3\n");
    }

    #[test]
    fn test_a_deleted_test_is_detected_and_restored() {
        let fixture = fixture();
        let manifest = Manifest::capture(&fixture.root, &protected(), Some(&fixture.snapshot));
        std::fs::remove_file(fixture.root.join("tests/test_math.py")).unwrap();

        let detected = manifest.verify(&protected());
        let actual = manifest.restore(&detected);

        assert_eq!(actual.violations[0].kind, ViolationKind::Deleted);
        assert!(actual.violations[0].restored);
        assert!(fixture.root.join("tests/test_math.py").exists());
    }

    #[test]
    fn test_a_newly_added_test_is_a_violation_and_is_removed() {
        let fixture = fixture();
        let manifest = Manifest::capture(&fixture.root, &protected(), Some(&fixture.snapshot));
        std::fs::write(fixture.root.join("tests/test_extra.py"), b"assert True\n").unwrap();

        let mut present = protected();
        present.push("tests/test_extra.py".to_string());
        let detected = manifest.verify(&present);
        let actual = manifest.restore(&detected);

        assert_eq!(actual.violations[0].kind, ViolationKind::Added);
        assert!(actual.violations[0].restored);
        assert!(!fixture.root.join("tests/test_extra.py").exists());
    }

    #[test]
    fn test_restoring_without_a_snapshot_reports_but_does_not_restore() {
        let fixture = fixture();
        let manifest = Manifest::capture(&fixture.root, &protected(), None);
        std::fs::write(fixture.root.join("tests/test_math.py"), b"assert True\n").unwrap();

        let detected = manifest.verify(&protected());
        let actual = manifest.restore(&detected);

        assert!(!actual.is_clean());
        assert!(!actual.violations[0].restored);
    }
}
