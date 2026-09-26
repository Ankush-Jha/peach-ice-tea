//! Catching shell commands that would mutate a protected test.
//!
//! The tool-layer guard covers the file tools, but `shell` can reach the same
//! files. Parsing shell properly is out of scope and would still be defeatable,
//! so this is a deliberately conservative screen for the obvious forms; the
//! after-the-run hash comparison remains the authoritative check.

use super::globs::ProtectedSet;

/// Programs that modify or delete the files named after them.
const MUTATING_PROGRAMS: &[&str] = &[
    "rm", "mv", "cp", "truncate", "shred", "dd", "install", "patch", "tee",
];

/// Redirections that rewrite a file, as opposed to reading one.
const WRITE_REDIRECTS: &[&str] = &[">", ">>", ">|"];

/// Refusal text when a command would mutate a protected path, else `None`.
///
/// Errs toward refusing: a false refusal costs one turn and says exactly why,
/// while a missed mutation risks the competition.
pub fn check_command(protected: &ProtectedSet, command: &str, cwd: &std::path::Path) -> Option<String> {
    let offending = mutated_paths(command)
        .into_iter()
        .rfind(|target| is_protected_token(protected, target, cwd))?;
    Some(format!(
        "Refused: this command would modify or delete `{offending}`, a protected test file.\n\
         The evaluation rules forbid modifying, deleting, renaming, disabling or skipping tests, \
         and test files are compared before and after this run.\n\
         Run the tests to check your work, and change the source code so the existing tests pass."
    ))
}

/// Every path a command would write or delete, as written in the command:
/// write-redirect targets, and the non-flag arguments of mutating programs
/// (`rm`, `mv`, `cp`, `tee`, `sed -i`, `git rm`, `git checkout --`, ...).
/// The same conservative screen as [`check_command`], without the protected
/// filter, so the verify gate can notice source edits made through `shell`.
pub fn mutated_paths(command: &str) -> Vec<String> {
    let tokens: Vec<String> = tokenize(command);
    let mut paths = Vec::new();

    // A write redirect targets the token immediately after it.
    for (index, token) in tokens.iter().enumerate() {
        if WRITE_REDIRECTS.contains(&token.as_str())
            && let Some(target) = tokens.get(index + 1)
        {
            paths.push(target.clone());
        }
    }

    // Segment on separators so `cd x && rm y` is screened per command.
    for segment in split_segments(&tokens) {
        let Some(program) = segment.first() else { continue };
        let program = program.rsplit('/').next().unwrap_or(program);

        let mutates = MUTATING_PROGRAMS.contains(&program)
            || (program == "git" && segment.get(1).map(String::as_str) == Some("rm"))
            || (program == "git"
                && segment.get(1).map(String::as_str) == Some("checkout")
                && segment.iter().any(|t| t == "--"))
            || (program == "sed" && segment.iter().any(|t| t == "-i" || t.starts_with("-i")))
            || (program == "perl" && segment.iter().any(|t| t.contains('i') && t.starts_with('-')));

        if mutates {
            paths.extend(
                segment
                    .iter()
                    .skip(1)
                    .filter(|token| !token.starts_with('-') && *token != "--")
                    .cloned(),
            );
        }
    }
    paths
}

fn is_protected_token(protected: &ProtectedSet, token: &str, cwd: &std::path::Path) -> bool {
    let cleaned = token.trim_matches(['"', '\'']);
    if cleaned.is_empty() {
        return false;
    }
    let path = std::path::Path::new(cleaned);
    let absolute = if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) };
    protected.is_protected(&absolute)
}

fn tokenize(command: &str) -> Vec<String> {
    // Keep redirects as their own tokens so `>file` and `> file` behave alike.
    let spaced = command
        .replace(">>", " >> ")
        .replace(">|", " >| ")
        .replace('|', " | ")
        .replace(';', " ; ")
        .replace("&&", " && ");
    let spaced = spaced.replace('>', " > ").replace(" >  > ", " >> ");
    spaced.split_whitespace().map(str::to_string).collect()
}

fn split_segments(tokens: &[String]) -> Vec<Vec<String>> {
    tokens
        .split(|token| matches!(token.as_str(), "&&" | "||" | ";" | "|"))
        .map(<[String]>::to_vec)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture() -> ProtectedSet {
        ProtectedSet::for_test(
            PathBuf::from("/repo"),
            vec![PathBuf::from("/repo/tests/test_math.py")],
        )
    }

    fn check(command: &str) -> Option<String> {
        check_command(&fixture(), command, Path::new("/repo"))
    }

    #[test]
    fn test_deleting_a_protected_test_is_refused() {
        assert!(check("rm tests/test_math.py").is_some());
    }

    #[test]
    fn test_overwriting_by_redirect_is_refused() {
        assert!(check("echo pass > tests/test_math.py").is_some());
        assert!(check("echo pass >> tests/test_math.py").is_some());
    }

    #[test]
    fn test_in_place_sed_is_refused() {
        assert!(check("sed -i '' 's/assert/pass/' tests/test_math.py").is_some());
    }

    #[test]
    fn test_git_checkout_of_a_protected_path_is_refused() {
        assert!(check("git checkout -- tests/test_math.py").is_some());
    }

    #[test]
    fn test_refusal_survives_a_compound_command() {
        assert!(check("cd /repo && rm tests/test_math.py").is_some());
    }

    #[test]
    fn test_running_the_tests_is_allowed() {
        assert_eq!(check("python -m pytest tests/test_math.py"), None);
        assert_eq!(check("cat tests/test_math.py"), None);
        assert_eq!(check("grep -n assert tests/test_math.py"), None);
    }

    #[test]
    fn test_editing_source_is_allowed() {
        assert_eq!(check("sed -i '' 's/0/3/' src/math.py"), None);
        assert_eq!(check("echo x > src/math.py"), None);
    }
}
