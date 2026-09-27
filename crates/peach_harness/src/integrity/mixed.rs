//! Test sections of mixed files (R-HACK-2, D-054).
//!
//! `package.json`, `pyproject.toml`, `Cargo.toml` and `setup.cfg` hold both
//! ordinary project settings and the test mechanism (the `test` script, jest
//! or pytest configuration, `[[test]]` targets). Protecting the whole file
//! would refuse legitimate work such as adding a dependency, so only the test
//! sections are fingerprinted before the run and compared after it. A change
//! is flagged as a violation; it is not restored, because restoring would mean
//! rewriting a file the task may have had good reason to edit elsewhere.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// File names whose test sections are watched, wherever they sit in the repo.
pub const MIXED_FILE_NAMES: &[&str] =
    &["package.json", "pyproject.toml", "Cargo.toml", "setup.cfg"];

/// `package.json` top-level keys that configure a test runner.
const PACKAGE_JSON_TEST_KEYS: &[&str] = &["jest", "mocha", "ava", "vitest", "c8", "nyc"];

/// The test sections of one mixed file, as captured before the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestSections {
    /// Path relative to the repository root.
    pub path: String,
    /// Canonical rendering of the file's test sections. Empty when the file
    /// has none, so adding one during the run is also a change.
    pub canonical: String,
}

/// Canonical rendering of the test sections in `content`, a file called
/// `file_name`. Unparseable content yields `None`: the caller cannot tell
/// what the test sections are, and says so rather than guessing.
///
/// # Arguments
/// * `file_name` - One of [`MIXED_FILE_NAMES`].
/// * `content` - The file's text.
pub fn extract(file_name: &str, content: &str) -> Option<String> {
    match file_name {
        "package.json" => extract_package_json(content),
        "pyproject.toml" => extract_toml(content, &[&["tool", "pytest"], &["tool", "tox"]]),
        "Cargo.toml" => extract_toml(content, &[&["test"], &["profile", "test"]]),
        "setup.cfg" => Some(extract_ini_sections(content, &["tool:pytest", "pytest"])),
        _ => None,
    }
}

fn extract_package_json(content: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let mut picked = serde_json::Map::new();
    if let Some(scripts) = json.get("scripts").and_then(serde_json::Value::as_object) {
        let tests: serde_json::Map<_, _> = scripts
            .iter()
            .filter(|(name, _)| name.contains("test"))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        if !tests.is_empty() {
            picked.insert("scripts".to_string(), serde_json::Value::Object(tests));
        }
    }
    for key in PACKAGE_JSON_TEST_KEYS {
        if let Some(value) = json.get(*key) {
            picked.insert((*key).to_string(), value.clone());
        }
    }
    Some(if picked.is_empty() {
        String::new()
    } else {
        serde_json::Value::Object(picked).to_string()
    })
}

fn extract_toml(content: &str, paths: &[&[&str]]) -> Option<String> {
    let document: toml_edit::DocumentMut = content.parse().ok()?;
    let mut parts = Vec::new();
    for path in paths {
        let mut item = document.as_item();
        let mut found = true;
        for key in *path {
            match item.get(key) {
                Some(next) => item = next,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if found {
            // Round-trip through serde so formatting and comments do not count as a change.
            let value: serde_json::Value =
                toml_edit::de::from_str(&item_as_document(item)?).ok()?;
            parts.push(format!("{}={}", path.join("."), value));
        }
    }
    Some(parts.join("\n"))
}

/// Wraps `item` as a standalone TOML document under the key `v`, so any item
/// (table, array of tables, value) can be deserialised the same way.
fn item_as_document(item: &toml_edit::Item) -> Option<String> {
    let mut document = toml_edit::DocumentMut::new();
    document.insert("v", item.clone());
    Some(document.to_string())
}

fn extract_ini_sections(content: &str, sections: &[&str]) -> String {
    let mut current: Option<String> = None;
    let mut lines = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            current = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')).map(|l| l.trim().to_string());
            continue;
        }
        let in_test_section = current
            .as_deref()
            .is_some_and(|name| sections.contains(&name));
        if in_test_section && !line.is_empty() && !line.starts_with('#') && !line.starts_with(';') {
            lines.push(format!("{}:{line}", current.as_deref().unwrap_or_default()));
        }
    }
    lines.join("\n")
}

/// Captures the test sections of every mixed file in `files` (paths relative
/// to `root`). Files that cannot be read or parsed are skipped with a warning:
/// the rest of the guard still runs.
///
/// # Arguments
/// * `root` - Repository root.
/// * `files` - Candidate paths relative to `root`.
pub fn capture(root: &Path, files: &[String]) -> Vec<TestSections> {
    files
        .iter()
        .filter_map(|relative| {
            let name = Path::new(relative).file_name()?.to_str()?;
            if !MIXED_FILE_NAMES.contains(&name) {
                return None;
            }
            let content = std::fs::read_to_string(root.join(relative)).ok()?;
            match extract(name, &content) {
                Some(canonical) => Some(TestSections { path: relative.clone(), canonical }),
                None => {
                    tracing::warn!(path = %relative, "Could not parse a mixed file; its test sections are not watched");
                    None
                }
            }
        })
        .collect()
}

/// The captured files whose test sections differ now. A file that was
/// deleted, or no longer parses, counts as changed when it had test sections.
///
/// # Arguments
/// * `root` - Repository root.
/// * `captured` - What [`capture`] recorded before the run.
pub fn changed(root: &Path, captured: &[TestSections]) -> Vec<String> {
    captured
        .iter()
        .filter(|before| {
            let name = Path::new(&before.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let now = std::fs::read_to_string(root.join(&before.path))
                .ok()
                .and_then(|content| extract(name, &content));
            match now {
                Some(canonical) => canonical != before.canonical,
                None => !before.canonical.is_empty(),
            }
        })
        .map(|before| before.path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const PACKAGE: &str = r#"{"name":"x","scripts":{"test":"node --test","build":"tsc"},"jest":{"bail":true},"dependencies":{}}"#;

    #[test]
    fn test_package_json_changes_outside_test_sections_are_ignored() {
        let fixture = PACKAGE
            .replace(
                r#""dependencies":{}"#,
                r#""dependencies":{"left-pad":"1.0.0"}"#,
            )
            .replace(r#""build":"tsc""#, r#""build":"tsc -b""#);

        let actual = extract("package.json", &fixture);

        let expected = extract("package.json", PACKAGE);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_package_json_test_script_and_runner_config_are_watched() {
        let fixture = [
            PACKAGE.replace("node --test", "node --test || true"),
            PACKAGE.replace(r#""bail":true"#, r#""bail":false"#),
            PACKAGE.replace(r#""build":"tsc""#, r#""build":"tsc","pretest":"exit 0""#),
        ];

        let actual: Vec<bool> = fixture
            .iter()
            .map(|c| extract("package.json", c) != extract("package.json", PACKAGE))
            .collect();

        let expected = vec![true, true, true];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_pyproject_pytest_table_is_watched_and_formatting_is_not() {
        let fixture = "[project]\nname = \"x\"\n\n[tool.pytest.ini_options]\naddopts = \"-q\"\n";
        let reformatted = "[project]\nname = \"x\"\n\n[tool.pytest.ini_options]\n# a comment\naddopts    =   \"-q\"\n";
        let skipping = "[project]\nname = \"x\"\n\n[tool.pytest.ini_options]\naddopts = \"-q -k 'not slow'\"\n";
        let new_dependency = "[project]\nname = \"x\"\ndependencies = [\"attrs\"]\n\n[tool.pytest.ini_options]\naddopts = \"-q\"\n";

        let before = extract("pyproject.toml", fixture);
        let actual =
            [reformatted, skipping, new_dependency].map(|c| extract("pyproject.toml", c) != before);

        let expected = [false, true, false];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_cargo_test_targets_are_watched() {
        let fixture =
            "[package]\nname = \"x\"\n\n[[test]]\nname = \"it\"\npath = \"tests/it.rs\"\n";
        let disabled = "[package]\nname = \"x\"\n\n[[test]]\nname = \"it\"\npath = \"tests/it.rs\"\ntest = false\n";

        let actual = extract("Cargo.toml", fixture) != extract("Cargo.toml", disabled);

        assert!(actual);
    }

    #[test]
    fn test_setup_cfg_pytest_section_only() {
        let fixture = "[metadata]\nname = x\n\n[tool:pytest]\naddopts = -q\n";

        let actual = extract("setup.cfg", fixture);

        let expected = Some("tool:pytest:addopts = -q".to_string());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_changed_reports_only_files_whose_test_sections_moved() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("package.json"), PACKAGE).unwrap();
        std::fs::create_dir_all(root.path().join("web")).unwrap();
        std::fs::write(root.path().join("web/package.json"), PACKAGE).unwrap();
        let files = vec![
            "package.json".to_string(),
            "web/package.json".to_string(),
            "README.md".to_string(),
        ];
        let captured = capture(root.path(), &files);
        std::fs::write(
            root.path().join("package.json"),
            PACKAGE.replace("tsc", "tsc -b"),
        )
        .unwrap();
        std::fs::write(
            root.path().join("web/package.json"),
            PACKAGE.replace("node --test", "true"),
        )
        .unwrap();

        let actual = (captured.len(), changed(root.path(), &captured));

        let expected = (2, vec!["web/package.json".to_string()]);
        assert_eq!(actual, expected);
    }
}
