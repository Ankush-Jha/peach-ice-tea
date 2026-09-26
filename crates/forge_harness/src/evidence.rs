//! The evidence bundle for one run (R-HACK-5, HACKATHON.md §13 and §30).
//!
//! One directory per run, holding everything a judge needs to review it
//! without re-running anything: the frozen prompt, the transcript, the
//! telemetry, the integrity result, the repository diff, the exec outcome
//! and a manifest of checksums. Everything written here is redacted first
//! (`R-SAFE-3`), and every write fails open: a file that cannot be written is
//! listed as missing in the manifest, and the run's outcome is unaffected
//! (`CLAUDE.md` principle 5).

use std::path::{Path, PathBuf};
use std::process::Command;

use bstr::ByteSlice;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::identity::HarnessIdentity;

/// Version of this directory layout, recorded in the manifest.
pub const EVIDENCE_LAYOUT_VERSION: &str = "0.1.0";

/// The frozen prompt, exactly as given to `exec` (redacted).
pub const PROMPT: &str = "prompt.txt";
/// The conversation and its subagent conversations.
pub const TRANSCRIPT: &str = "transcript.json";
/// The telemetry event stream.
pub const TELEMETRY: &str = "telemetry.jsonl";
/// The post-run test-integrity result.
pub const INTEGRITY: &str = "integrity.json";
/// Every repository change against the commit the run started from.
pub const DIFF: &str = "diff.patch";
/// The harness's own final test run.
pub const TESTS: &str = "tests.json";
/// The `exec` outcome line.
pub const EXEC: &str = "exec.json";
/// Checksums of every other file, written last.
pub const MANIFEST: &str = "manifest.json";

/// Writes the files of one evidence bundle.
#[derive(Debug, Clone)]
pub struct Evidence {
    dir: PathBuf,
    notes: Vec<String>,
}

impl Evidence {
    /// Creates the evidence directory. Returns `None` (with a warning) when it
    /// cannot be created: the run continues without a bundle.
    ///
    /// # Arguments
    /// * `dir` - Directory to write the bundle into; created if missing.
    pub fn create(dir: impl Into<PathBuf>) -> Option<Self> {
        let dir = dir.into();
        match std::fs::create_dir_all(&dir) {
            Ok(()) => Some(Self { dir, notes: vec![] }),
            Err(error) => {
                tracing::warn!(?error, dir = %dir.display(), "Cannot create evidence dir; no bundle");
                None
            }
        }
    }

    /// The bundle directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Path of a file inside the bundle.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Records a fact about the bundle for the manifest, such as why a file
    /// is absent. Absence without a reason reads as a harness bug.
    pub fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// Writes redacted text.
    pub fn write_text(&mut self, name: &str, text: &str) {
        let redacted = crate::redact::redact(text);
        self.write_bytes(name, redacted.as_bytes());
    }

    /// Writes a value as pretty JSON, with every string leaf redacted.
    pub fn write_json(&mut self, name: &str, value: &impl Serialize) {
        match serde_json::to_value(value) {
            Ok(mut json) => {
                crate::redact::redact_json_strings(&mut json);
                let text = serde_json::to_string_pretty(&json).unwrap_or_default();
                self.write_bytes(name, text.as_bytes());
            }
            Err(error) => self.note(format!("{name}: not written, could not serialise: {error}")),
        }
    }

    fn write_bytes(&mut self, name: &str, bytes: &[u8]) {
        if let Err(error) = std::fs::write(self.path(name), bytes) {
            tracing::warn!(?error, name, "Could not write evidence file");
            self.note(format!("{name}: not written: {error}"));
        }
    }

    /// Writes `diff.patch`: every change in `repo` against `start`, including
    /// untracked files, excluding the bundle itself when it lies inside the
    /// repository. Built with a temporary index so the repository's own index
    /// and working tree are untouched, and against the start commit so the
    /// agent's own commits cannot hide changes.
    ///
    /// # Arguments
    /// * `repo` - Repository root.
    /// * `start` - Commit the run started from; `None` when the repository
    ///   was not a git repository or had no commit.
    pub fn write_diff(&mut self, repo: &Path, start: Option<&str>) {
        let Some(start) = start else {
            self.note(format!("{DIFF}: not written, the repository had no git commit to diff against"));
            return;
        };
        match git_diff_against(repo, start, self.dir.strip_prefix(repo).ok()) {
            Ok(diff) => self.write_text(DIFF, &diff),
            Err(error) => self.note(format!("{DIFF}: not written: {error}")),
        }
    }

    /// Writes `manifest.json` with the SHA-256 of every file in the bundle.
    /// Call last: files written afterwards are not covered.
    ///
    /// # Arguments
    /// * `started_at` - RFC 3339 time the run started.
    /// * `outcome` - How the run ended.
    pub fn write_manifest(&mut self, started_at: &str, outcome: &str) {
        let files = hash_files(&self.dir);
        let manifest = Manifest {
            harness: HarnessIdentity::current(),
            evidence_layout_version: EVIDENCE_LAYOUT_VERSION,
            telemetry_schema_version: crate::identity::TELEMETRY_SCHEMA_VERSION,
            started_at: started_at.to_string(),
            ended_at: now(),
            outcome: outcome.to_string(),
            files,
            notes: self.notes.clone(),
        };
        let text = serde_json::to_string_pretty(&manifest).unwrap_or_default();
        if let Err(error) = std::fs::write(self.path(MANIFEST), text) {
            tracing::warn!(?error, "Could not write evidence manifest");
        }
    }
}

#[derive(Serialize)]
struct Manifest {
    harness: HarnessIdentity,
    evidence_layout_version: &'static str,
    telemetry_schema_version: &'static str,
    started_at: String,
    ended_at: String,
    outcome: String,
    /// Bundle-relative path to SHA-256, for every file except the manifest.
    files: std::collections::BTreeMap<String, String>,
    /// Why anything expected is missing.
    notes: Vec<String>,
}

/// Current time, RFC 3339 with milliseconds.
pub fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// The commit `repo` is at, or `None` when it is not a git repository or has
/// no commits yet.
pub fn git_head(repo: &Path) -> Option<String> {
    let output = Command::new("git").args(["rev-parse", "--verify", "HEAD"]).current_dir(repo).output().ok()?;
    output.status.success().then(|| output.stdout.to_str_lossy().trim().to_string())
}

fn git_diff_against(repo: &Path, start: &str, exclude: Option<&Path>) -> anyhow::Result<String> {
    let index = tempfile::NamedTempFile::new()?;
    // An empty file is not a valid index; git creates one at this path.
    let index_path = index.path().to_path_buf();
    std::fs::remove_file(&index_path)?;

    let mut add = Command::new("git");
    add.args(["add", "-A", "--", "."]).current_dir(repo).env("GIT_INDEX_FILE", &index_path);
    if let Some(exclude) = exclude {
        add.arg(format!(":(exclude){}", exclude.display()));
    }
    let added = add.output()?;
    anyhow::ensure!(added.status.success(), "git add failed: {}", added.stderr.to_str_lossy());

    let diff = Command::new("git")
        .args(["diff", "--cached", "--binary", start])
        .current_dir(repo)
        .env("GIT_INDEX_FILE", &index_path)
        .output()?;
    anyhow::ensure!(diff.status.success(), "git diff failed: {}", diff.stderr.to_str_lossy());
    let _ = std::fs::remove_file(&index_path);
    Ok(diff.stdout.to_str_lossy().into_owned())
}

fn hash_files(dir: &Path) -> std::collections::BTreeMap<String, String> {
    let mut files = std::collections::BTreeMap::new();
    for entry in ignore::WalkBuilder::new(dir).standard_filters(false).build().filter_map(Result::ok) {
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(dir) else { continue };
        let name = relative.to_string_lossy().replace('\\', "/");
        if name == MANIFEST {
            continue;
        }
        if let Ok(bytes) = std::fs::read(entry.path()) {
            let digest = Sha256::digest(&bytes);
            files.insert(name, digest.iter().map(|byte| format!("{byte:02x}")).collect());
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn git(repo: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args(args)
            .current_dir(repo)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn fixture_repo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init", "-q"]);
        std::fs::write(repo.path().join("a.py"), "x = 1\n").unwrap();
        git(repo.path(), &["add", "-A"]);
        git(repo.path(), &["commit", "-qm", "start"]);
        repo
    }

    #[test]
    fn test_diff_covers_edits_new_files_and_commits_without_touching_the_index() {
        let repo = fixture_repo();
        let start = git_head(repo.path()).unwrap();
        // The agent edits, commits, then leaves an untracked file behind.
        std::fs::write(repo.path().join("a.py"), "x = 2\n").unwrap();
        git(repo.path(), &["commit", "-qam", "agent commit"]);
        std::fs::write(repo.path().join("new.py"), "y = 1\n").unwrap();
        let status_before = Command::new("git").args(["status", "--porcelain"]).current_dir(repo.path()).output().unwrap().stdout;
        let dir = repo.path().join("evidence");
        let mut fixture = Evidence::create(&dir).unwrap();

        fixture.write_diff(repo.path(), Some(&start));

        let actual = std::fs::read_to_string(dir.join(DIFF)).unwrap();
        assert!(actual.contains("-x = 1") && actual.contains("+x = 2"), "{actual}");
        assert!(actual.contains("+y = 1"), "untracked file missing:\n{actual}");
        assert!(!actual.contains("evidence/"), "the bundle diffed itself:\n{actual}");
        let status_after = Command::new("git").args(["status", "--porcelain"]).current_dir(repo.path()).output().unwrap().stdout;
        // Only the evidence dir itself is new; nothing was staged.
        assert_eq!(
            status_after.to_str_lossy().replace("?? evidence/\n", ""),
            status_before.to_str_lossy()
        );
    }

    #[test]
    fn test_manifest_checksums_every_file_and_explains_what_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let mut fixture = Evidence::create(dir.path()).unwrap();
        fixture.write_text(PROMPT, "fix the bug");
        fixture.write_diff(dir.path(), None);

        fixture.write_manifest("2026-09-25T00:00:00.000Z", "completed");

        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join(MANIFEST)).unwrap()).unwrap();
        let expected_hash: String =
            Sha256::digest(b"fix the bug").iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(manifest["files"], serde_json::json!({ PROMPT: expected_hash }));
        assert_eq!(manifest["outcome"], "completed");
        assert!(manifest["notes"][0].as_str().unwrap().starts_with("diff.patch: not written"));
    }

    #[test]
    fn test_written_files_are_redacted() {
        let dir = tempfile::tempdir().unwrap();
        let mut fixture = Evidence::create(dir.path()).unwrap();
        let key = format!("AIza{}", "z".repeat(35));

        fixture.write_text(PROMPT, &format!("key {key}"));
        fixture.write_json(EXEC, &serde_json::json!({"error": format!("bad key {key}"), "input_tokens": 7}));

        let prompt = std::fs::read_to_string(dir.path().join(PROMPT)).unwrap();
        let exec = std::fs::read_to_string(dir.path().join(EXEC)).unwrap();
        assert!(!prompt.contains(&key) && !exec.contains(&key));
        assert!(exec.contains("\"input_tokens\": 7"));
    }
}
