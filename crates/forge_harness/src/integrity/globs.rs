//! Deciding which paths count as tests.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use glob::Pattern;

/// Patterns treated as tests when the configuration does not say otherwise.
///
/// Deliberately broad. A false positive costs the model one refused edit to a
/// file it probably should not have touched; a false negative can cost the
/// competition. Covers the common layouts for Python, JavaScript/TypeScript,
/// Rust, Go and Java, plus snapshot files, which are tests in everything but
/// name — "just update the snapshot" is the same cheat as editing an assertion.
pub const DEFAULT_PROTECTED_GLOBS: &[&str] = &[
    "**/tests/**",
    "**/test/**",
    "**/__tests__/**",
    "**/spec/**",
    "**/testdata/**",
    "**/*_test.*",
    "**/test_*.*",
    "**/*.test.*",
    "**/*.spec.*",
    "**/*Test.java",
    "**/*Tests.java",
    "**/conftest.py",
    "**/__snapshots__/**",
    "**/*.snap",
    "**/pytest.ini",
    "**/tox.ini",
    "**/jest.config.*",
    "**/vitest.config.*",
    "**/playwright.config.*",
    "**/karma.conf.*",
];

/// Paths never treated as tests, however they match above.
///
/// Build outputs and dependency trees contain vendored test files that are not
/// the repository's own tests; protecting them would refuse legitimate work and
/// make the manifest enormous.
pub const DEFAULT_EXCLUDE_GLOBS: &[&str] = &[
    "**/.git/**",
    "**/node_modules/**",
    "**/target/**",
    "**/.venv/**",
    "**/venv/**",
    "**/__pycache__/**",
    "**/*.pyc",
    "**/.pytest_cache/**",
    "**/dist/**",
    "**/build/**",
    "**/coverage/**",
    "**/.tox/**",
];

/// Canonical form of `path`, resolving symlinks and `..` through its deepest
/// existing ancestor, so a file that does not exist yet (a test the agent is
/// about to create) still resolves.
fn canonicalize_lenient(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = path.canonicalize() {
        return Some(canonical);
    }
    let parent = path.parent()?;
    let name = path.file_name()?;
    Some(canonicalize_lenient(parent)?.join(name))
}

/// The set of protected paths for one repository.
#[derive(Debug, Clone)]
pub struct ProtectedSet {
    root: PathBuf,
    files: BTreeSet<String>,
    protect: Vec<Pattern>,
    exclude: Vec<Pattern>,
}

impl ProtectedSet {
    /// Builds a set from glob patterns and the files found under `root`.
    ///
    /// Unparseable patterns are skipped with a warning rather than failing:
    /// losing one pattern is better than losing the whole guard (`CLAUDE.md`
    /// principle 5).
    pub fn new(root: impl Into<PathBuf>, protect: &[String], exclude: &[String], files: Vec<PathBuf>) -> Self {
        let compile = |patterns: &[String]| -> Vec<Pattern> {
            patterns
                .iter()
                .filter_map(|p| match Pattern::new(p) {
                    Ok(pattern) => Some(pattern),
                    Err(error) => {
                        tracing::warn!(pattern = %p, ?error, "Ignoring unparseable protected-path glob");
                        None
                    }
                })
                .collect()
        };
        let root = root.into();
        let protect = compile(protect);
        let exclude = compile(exclude);

        let mut set = Self { root, files: BTreeSet::new(), protect, exclude };
        set.files = files
            .into_iter()
            .filter(|path| set.matches(path))
            .map(|path| set.display_path(&path))
            .collect();
        set
    }

    /// Builds a set protecting exactly the given files, for tests.
    ///
    /// Carries no patterns, so only the listed files are protected and
    /// everything else is allowed.
    pub fn for_test(root: impl Into<PathBuf>, files: Vec<PathBuf>) -> Self {
        let root = root.into();
        let mut set = Self { root, files: BTreeSet::new(), protect: vec![], exclude: vec![] };
        set.files = files.iter().map(|path| set.display_path(path)).collect();
        set
    }

    /// Path relative to the repository root, for display and comparison.
    ///
    /// Falls back to comparing canonical forms when the path does not start
    /// with `root` as written — a symlinked prefix (macOS's `/var` is
    /// `/private/var`) or `..` components. Without that fallback such a path
    /// stayed absolute: the recorded-file lookup missed, and an exclude glob
    /// could match a directory *above* the repository (a repo under `build/`
    /// matches `**/build/**`), letting a real test through.
    pub fn display_path(&self, path: &Path) -> String {
        let relative = path
            .strip_prefix(&self.root)
            .ok()
            .map(Path::to_path_buf)
            .or_else(|| {
                let root = self.root.canonicalize().ok()?;
                canonicalize_lenient(path)?.strip_prefix(root).ok().map(Path::to_path_buf)
            });
        relative
            .as_deref()
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Whether a path matches the protect patterns and no exclude pattern.
    fn matches(&self, path: &Path) -> bool {
        let relative = self.display_path(path);
        let candidate = Path::new(&relative);
        if self.exclude.iter().any(|p| p.matches_path(candidate)) {
            return false;
        }
        self.protect.iter().any(|p| p.matches_path(candidate))
    }

    /// Whether this exact path is protected.
    ///
    /// Checked against the recorded file list first so a path that was present
    /// at capture time stays protected even if it is later deleted, and falls
    /// back to pattern matching so newly created test files are caught too.
    pub fn is_protected(&self, path: &Path) -> bool {
        let relative = self.display_path(path);
        self.files.contains(&relative) || self.matches(path)
    }

    /// Protected files, relative to the repository root.
    pub fn protected_files(&self) -> Vec<String> {
        self.files.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_a_path_spelled_through_a_symlink_is_still_protected_and_relative() {
        // The repo really lives under `build/`, which an exclude glob matches,
        // and the harness knows it by a symlinked alias. A tool call naming
        // the real path used to stay absolute, hit `**/build/**`, and slip
        // through as unprotected.
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("build/repo");
        std::fs::create_dir_all(real.join("tests")).unwrap();
        std::fs::write(real.join("tests/test_x.py"), "").unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let protect: Vec<String> = DEFAULT_PROTECTED_GLOBS.iter().map(|s| s.to_string()).collect();
        let exclude: Vec<String> = DEFAULT_EXCLUDE_GLOBS.iter().map(|s| s.to_string()).collect();
        let fixture = ProtectedSet::new(&alias, &protect, &exclude, vec![alias.join("tests/test_x.py")]);

        let existing = real.join("tests/test_x.py");
        let not_yet_created = real.join("tests/test_new.py");
        let actual = (
            fixture.is_protected(&existing),
            fixture.display_path(&existing),
            fixture.is_protected(&not_yet_created),
            fixture.display_path(&not_yet_created),
        );

        let expected = (
            true,
            "tests/test_x.py".to_string(),
            true,
            "tests/test_new.py".to_string(),
        );
        assert_eq!(actual, expected);
    }

    fn fixture_set(files: &[&str]) -> ProtectedSet {
        let protect: Vec<String> = DEFAULT_PROTECTED_GLOBS.iter().map(|s| s.to_string()).collect();
        let exclude: Vec<String> = DEFAULT_EXCLUDE_GLOBS.iter().map(|s| s.to_string()).collect();
        ProtectedSet::new(
            "/repo",
            &protect,
            &exclude,
            files.iter().map(|f| PathBuf::from("/repo").join(f)).collect(),
        )
    }

    #[test]
    fn test_common_test_layouts_are_protected() {
        let fixture = fixture_set(&[
            "tests/test_math.py",
            "src/math.py",
            "src/util_test.go",
            "web/__tests__/app.test.ts",
            "api/conftest.py",
            "ui/__snapshots__/App.test.tsx.snap",
            "README.md",
        ]);

        let actual = fixture.protected_files();
        let expected = vec![
            "api/conftest.py".to_string(),
            "src/util_test.go".to_string(),
            "tests/test_math.py".to_string(),
            "ui/__snapshots__/App.test.tsx.snap".to_string(),
            "web/__tests__/app.test.ts".to_string(),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_vendored_tests_are_excluded() {
        let fixture = fixture_set(&[
            "node_modules/left-pad/test/index.test.js",
            "target/debug/build/thing/tests/x_test.rs",
            "tests/test_real.py",
        ]);

        let actual = fixture.protected_files();
        let expected = vec!["tests/test_real.py".to_string()];

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_a_new_test_file_is_protected_even_though_it_was_not_captured() {
        let fixture = fixture_set(&["src/math.py"]);

        let actual = fixture.is_protected(&PathBuf::from("/repo/tests/test_sneaky.py"));

        assert!(actual);
    }

    #[test]
    fn test_source_files_are_not_protected() {
        let fixture = fixture_set(&["tests/test_math.py", "src/math.py"]);

        let actual = fixture.is_protected(&PathBuf::from("/repo/src/math.py"));

        assert!(!actual);
    }
}
