//! Which command runs a repository's tests (R-HACK-7).
//!
//! An explicit command always wins: the frozen prompt usually names one, and
//! the eval runner passes it through `exec --test-command`. Detection is the
//! fallback, from files at the repository root, in a fixed order.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// A command that runs the repository's tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCommand {
    /// The shell command line.
    pub command: String,
    /// Why this command: `explicit`, or the marker file that implied it.
    pub source: String,
}

impl TestCommand {
    fn new(command: impl Into<String>, source: impl Into<String>) -> Self {
        Self { command: command.into(), source: source.into() }
    }
}

/// The test command for `root`: `explicit` when given, else detected.
///
/// # Arguments
/// * `root` - Repository root.
/// * `explicit` - A command supplied by the caller, which always wins.
pub fn detect(root: &Path, explicit: Option<&str>) -> Option<TestCommand> {
    if let Some(command) = explicit.map(str::trim).filter(|c| !c.is_empty()) {
        return Some(TestCommand::new(command, "explicit"));
    }
    let exists = |name: &str| root.join(name).exists();
    let read = |name: &str| std::fs::read_to_string(root.join(name)).unwrap_or_default();

    if exists("Cargo.toml") {
        return Some(TestCommand::new("cargo test", "Cargo.toml"));
    }
    if exists("package.json") {
        let package: serde_json::Value = serde_json::from_str(&read("package.json")).unwrap_or_default();
        let script = package.get("scripts").and_then(|s| s.get("test")).and_then(|t| t.as_str()).unwrap_or_default();
        // `npm init`'s placeholder fails on purpose; it is not a test suite.
        if !script.is_empty() && !script.contains("no test specified") {
            let runner = if exists("pnpm-lock.yaml") {
                "pnpm test"
            } else if exists("yarn.lock") {
                "yarn test"
            } else {
                "npm test"
            };
            return Some(TestCommand::new(runner, "package.json scripts.test"));
        }
        if has_file_matching(root, |name| name.ends_with(".test.js") || name.ends_with(".test.mjs")) {
            return Some(TestCommand::new("node --test", "*.test.js files"));
        }
    }
    let pytest_marker = ["pytest.ini", "conftest.py", "tests/conftest.py"]
        .into_iter()
        .find(|name| exists(name))
        .map(str::to_string)
        .or_else(|| read("pyproject.toml").contains("[tool.pytest").then(|| "pyproject.toml".to_string()))
        .or_else(|| read("setup.cfg").contains("[tool:pytest]").then(|| "setup.cfg".to_string()))
        .or_else(|| read("tox.ini").contains("[pytest]").then(|| "tox.ini".to_string()));
    if let Some(marker) = pytest_marker {
        return Some(TestCommand::new("python3 -m pytest -q", marker));
    }
    // Python tests with no pytest configuration: the standard library's
    // runner is always installed, pytest may not be.
    if exists("tests") && has_file_matching(&root.join("tests"), is_python_test) {
        return Some(TestCommand::new(
            "python3 -m unittest discover -s tests -t . -v",
            "tests/test_*.py",
        ));
    }
    if has_file_matching(root, is_python_test) {
        return Some(TestCommand::new("python3 -m unittest discover -v", "test_*.py"));
    }
    if exists("go.mod") {
        return Some(TestCommand::new("go test ./...", "go.mod"));
    }
    if read("Makefile").lines().any(|line| line.starts_with("test:")) {
        return Some(TestCommand::new("make test", "Makefile test target"));
    }
    if exists("pom.xml") {
        return Some(TestCommand::new("mvn -q test", "pom.xml"));
    }
    if exists("gradlew") {
        return Some(TestCommand::new("./gradlew test", "gradlew"));
    }
    None
}

fn is_python_test(name: &str) -> bool {
    name.ends_with(".py") && (name.starts_with("test_") || name.ends_with("_test.py"))
}

/// Whether any file directly in `dir`, or one level below it, matches.
fn has_file_matching(dir: &Path, matches: impl Fn(&str) -> bool) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            !name.starts_with('.')
                && name != "node_modules"
                && std::fs::read_dir(&path).is_ok_and(|inner| {
                    inner
                        .filter_map(Result::ok)
                        .any(|e| matches(&e.file_name().to_string_lossy()))
                })
        } else {
            matches(&name)
        }
    })
}

/// Whether a shell command runs tests: one of its segments *runs* the
/// detected command or a known test runner. Mentioning a runner is not
/// running it: `grep -r pytest .` is not a test run, and must not satisfy
/// the completion gate.
///
/// # Arguments
/// * `command` - The shell command line the agent ran.
/// * `detected` - The repository's test command, when known.
pub fn is_test_command(command: &str, detected: Option<&TestCommand>) -> bool {
    const RUNNERS: &[&str] = &[
        "pytest",
        "python -m pytest",
        "python3 -m pytest",
        "python -m unittest",
        "python3 -m unittest",
        "cargo test",
        "cargo nextest",
        "npm test",
        "npm run test",
        "yarn test",
        "pnpm test",
        "node --test",
        "go test",
        "jest",
        "vitest",
        "mvn test",
        "mvn -q test",
        "gradle test",
        "./gradlew test",
        "make test",
        "tox",
    ];
    let detected = detected.map(|test| normalise(&test.command));
    command.split(['&', ';', '|']).any(|segment| {
        let run = invoked(segment);
        if run.is_empty() {
            return false;
        }
        detected.as_deref().is_some_and(|test| run.starts_with(test))
            || RUNNERS.iter().any(|runner| run == *runner || run.starts_with(&format!("{runner} ")))
    })
}

fn normalise(command: &str) -> String {
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The program a command segment runs, with leading `VAR=value`
/// assignments and wrappers (`timeout 60`, `npx`, `uv run`, ...) removed.
fn invoked(segment: &str) -> String {
    const WRAPPERS: &[&str] = &["timeout", "npx", "uv", "poetry", "pipenv", "run", "exec", "env", "command"];
    let tokens: Vec<&str> = segment.split_whitespace().collect();
    let start = tokens
        .iter()
        .position(|token| {
            !(token.contains('=') && !token.starts_with('-'))
                && !WRAPPERS.contains(token)
                && token.parse::<f64>().is_err()
        })
        .unwrap_or(tokens.len());
    tokens.get(start..).unwrap_or_default().join(" ")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, content) in files {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        dir
    }

    fn detected(files: &[(&str, &str)]) -> Option<String> {
        let dir = fixture(files);
        detect(dir.path(), None).map(|test| test.command)
    }

    #[test]
    fn test_detection_table() {
        let actual = vec![
            detected(&[("Cargo.toml", "[package]")]),
            detected(&[("package.json", r#"{"scripts":{"test":"jest"}}"#), ("yarn.lock", "")]),
            detected(&[("package.json", r#"{"scripts":{"test":"echo \"Error: no test specified\" && exit 1"}}"#), ("lib.test.js", "")]),
            detected(&[("pyproject.toml", "[tool.pytest.ini_options]"), ("tests/test_a.py", "")]),
            detected(&[("tests/__init__.py", ""), ("tests/test_stats.py", "")]),
            detected(&[("test_x.py", "")]),
            detected(&[("go.mod", "module x")]),
            detected(&[("Makefile", "build:\n\ttrue\ntest:\n\ttrue\n")]),
            detected(&[("README.md", "")]),
        ];
        let expected: Vec<Option<String>> = vec![
            Some("cargo test".into()),
            Some("yarn test".into()),
            Some("node --test".into()),
            Some("python3 -m pytest -q".into()),
            Some("python3 -m unittest discover -s tests -t . -v".into()),
            Some("python3 -m unittest discover -v".into()),
            Some("go test ./...".into()),
            Some("make test".into()),
            None,
        ];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_an_explicit_command_wins() {
        let dir = fixture(&[("Cargo.toml", "")]);

        let actual = detect(dir.path(), Some("  make check  "));

        assert_eq!(actual, Some(TestCommand::new("make check", "explicit")));
    }

    #[test]
    fn test_test_commands_are_recognised_and_others_are_not() {
        let detected = TestCommand::new("python3 -m unittest discover -s tests -t . -v", "explicit");
        let yes = [
            "python3 -m unittest discover -s tests -t . -v",
            "cd repo && python -m pytest -x tests/test_a.py",
            "cargo  test --offline",
            "npm test 2>&1 | tail -20",
        ];
        let wrapped = ["timeout 60 npx jest", "CI=1 uv run pytest -q"];
        let no = ["cat tests/test_a.py", "grep -r pytest .", "python3 stats.py", "echo run the tests"];

        let actual: Vec<bool> = yes
            .iter()
            .chain(wrapped.iter())
            .chain(no.iter())
            .map(|c| is_test_command(c, Some(&detected)))
            .collect();

        assert_eq!(actual, vec![true, true, true, true, true, true, false, false, false, false]);
    }
}
