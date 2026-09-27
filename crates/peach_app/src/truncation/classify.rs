//! Shell command classification and lossless search regrouping (R-OUT-2, T1.4).
//!
//! The command line decides how its output is shaped: search output can be
//! regrouped without losing a match, noise (builds, installs, test runners)
//! can be compressed (T1.5, `compress_noise.rs`), and everything else keeps
//! peach's head/tail behaviour. Regrouping is behind
//! `PEACH_HARNESS_SEARCH_REGROUP=1`, default off until an A/B (D-073).

/// Environment variable that turns search regrouping on.
pub const SEARCH_REGROUP_ENV_VAR: &str = "PEACH_HARNESS_SEARCH_REGROUP";

/// Whether search regrouping is enabled for this process.
pub fn search_regroup_enabled() -> bool {
    std::env::var(SEARCH_REGROUP_ENV_VAR).as_deref() == Ok("1")
}

/// What a command's output is, for shaping purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputClass {
    /// `grep`, `rg`, `ag`, `find`, `fd`, `git grep`: lines of matches.
    Search,
    /// Builds, installs, test runners, linters: long, repetitive, with the
    /// few lines that matter buried inside.
    Noise,
    /// `cat`, `head`, `sed -n`, `git diff`, `jq`...: the content itself.
    SourceLike,
    /// Anything else: peach's existing head/tail behaviour.
    Unknown,
}

const SEARCH: &[&str] = &["grep", "egrep", "rg", "ag", "ack", "find", "fd"];
const NOISE: &[&str] = &[
    "npm", "pnpm", "yarn", "npx", "pip", "pip3", "uv", "poetry", "cargo", "go", "mvn", "gradle",
    "gradlew", "make", "cmake", "pytest", "tox", "nox", "jest", "vitest", "mocha", "eslint", "tsc",
    "ruff", "mypy", "flake8", "black", "prettier", "bundle", "rake", "dotnet", "swift", "composer",
];
const SOURCE_LIKE: &[&str] = &[
    "cat", "head", "tail", "sed", "bat", "less", "more", "jq", "nl", "awk",
];

/// The class of `command`, from the first program of each pipeline or list
/// segment. Search wins over noise, and noise over source-like, so
/// `cargo test | grep FAIL` is search and `cat log | tail` is source-like.
///
/// # Arguments
/// * `command` - The shell command line.
pub fn classify(command: &str) -> OutputClass {
    let classes: Vec<OutputClass> = segments(command)
        .iter()
        .map(|words| segment_class(words))
        .collect();
    [
        OutputClass::Search,
        OutputClass::Noise,
        OutputClass::SourceLike,
    ]
    .into_iter()
    .find(|class| classes.contains(class))
    .unwrap_or(OutputClass::Unknown)
}

fn segments(command: &str) -> Vec<Vec<&str>> {
    command
        .split(['|', ';', '&'])
        .map(|segment| segment.split_whitespace().collect::<Vec<_>>())
        .filter(|words| !words.is_empty())
        .collect()
}

fn segment_class(words: &[&str]) -> OutputClass {
    // Skip env assignments and wrappers to reach the program itself.
    let mut rest = words.iter().copied().skip_while(|w| {
        w.contains('=') && !w.starts_with('-')
            || matches!(*w, "sudo" | "time" | "env" | "exec" | "nice")
    });
    let Some(program) = rest.next() else {
        return OutputClass::Unknown;
    };
    let program = program.rsplit('/').next().unwrap_or(program);
    let next = rest.next();
    match (program, next) {
        ("git", Some("grep")) => OutputClass::Search,
        ("git", Some("diff" | "show" | "log" | "blame")) => OutputClass::SourceLike,
        ("python" | "python3", Some("-m")) => match rest.next() {
            Some("pytest" | "unittest" | "pip" | "mypy" | "ruff" | "tox") => OutputClass::Noise,
            _ => OutputClass::Unknown,
        },
        (p, _) if SEARCH.contains(&p) => OutputClass::Search,
        (p, _) if NOISE.contains(&p) => OutputClass::Noise,
        (p, _) if SOURCE_LIKE.contains(&p) => OutputClass::SourceLike,
        _ => OutputClass::Unknown,
    }
}

/// Search output (`path:line:text` or `path:text`, as `grep -rn`/`rg -n`
/// print it) regrouped under one header per file, losing nothing: every match
/// line stays, only the repeated path prefix goes. `None` when the output is
/// not in that shape or regrouping would not make it smaller.
///
/// # Arguments
/// * `output` - The search command's stdout.
pub fn regroup_search(output: &str) -> Option<String> {
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in output.lines() {
        let (path, rest) = line.split_once(':')?;
        if path.is_empty() || path.contains(' ') && !path.contains('/') {
            return None;
        }
        match groups.last_mut() {
            Some((last, lines)) if *last == path => lines.push(rest),
            _ => groups.push((path, vec![rest])),
        }
    }
    if groups.is_empty() {
        return None;
    }
    let regrouped: String = groups
        .iter()
        .map(|(path, lines)| {
            format!(
                "{path}\n{}",
                lines
                    .iter()
                    .map(|l| format!("  {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    (regrouped.len() < output.len()).then_some(regrouped)
}

/// The stream as the model should see it: noise compressed (T1.5) or search
/// regrouped (T1.4) when the command's class and the flag say so, otherwise
/// unchanged. Compressed output ends with the shared recovery sentence,
/// naming `full_output` (R-OUT-3, R-OUT-4).
///
/// # Arguments
/// * `command` - The shell command that produced `text`.
/// * `text` - stdout or stderr.
/// * `full_output` - Where the raw stream was saved, if it was.
pub fn shape_shell_stream<'a>(
    command: &str,
    text: &'a str,
    full_output: Option<&std::path::Path>,
) -> std::borrow::Cow<'a, str> {
    match classify(command) {
        OutputClass::Noise if super::noise_compression_enabled() => {
            match super::compress_noise(text) {
                Some((compressed, collapsed)) => {
                    let recovery = match full_output {
                        // A parenthetical, not a full stop, after the path: a period
                        // glued to a path gets read as part of it.
                        Some(path) => format!(
                            "Full output: read {} (the complete output).",
                            path.display()
                        ),
                        None => "The full output was not saved; re-run with a narrower command."
                            .to_string(),
                    };
                    let notice = super::recovery_notice(
                        collapsed as u64,
                        "similar or passing lines",
                        &recovery,
                    );
                    std::borrow::Cow::Owned(format!("{compressed}\n{notice}"))
                }
                None => std::borrow::Cow::Borrowed(text),
            }
        }
        OutputClass::Search if search_regroup_enabled() => {
            regroup_search(text).map_or(std::borrow::Cow::Borrowed(text), std::borrow::Cow::Owned)
        }
        _ => std::borrow::Cow::Borrowed(text),
    }
}

/// Whether [`shape_shell_stream`] would withhold part of `text`, so its full
/// form must be saved even when it is under the truncation caps.
pub fn shaping_withholds(command: &str, text: &str) -> bool {
    classify(command) == OutputClass::Noise
        && super::noise_compression_enabled()
        && super::compress_noise(text).is_some()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_commands_are_classified_by_their_programs() {
        let fixture = [
            "rg -n TODO src",
            "git grep -n foo",
            "cargo test 2>&1 | grep FAIL",
            "cd web && npm install",
            "RUST_LOG=debug cargo build",
            "python3 -m pytest -q",
            "cat src/lib.rs",
            "git diff HEAD~1",
            "./scripts/deploy.sh",
            "python3 script.py",
        ];

        let actual: Vec<OutputClass> = fixture.iter().map(|c| classify(c)).collect();

        use OutputClass::*;
        assert_eq!(
            actual,
            vec![
                Search, Search, Search, Noise, Noise, Noise, SourceLike, SourceLike, Unknown,
                Unknown
            ]
        );
    }

    #[test]
    fn test_search_output_is_regrouped_without_losing_a_match() {
        let fixture = "src/money.py:9:def to_money(value):\nsrc/money.py:14:def allocate(amount, ratios):\nsrc/invoice.py:3:from .money import to_money\n";

        let actual = regroup_search(fixture).unwrap();

        let expected = "src/money.py\n  9:def to_money(value):\n  14:def allocate(amount, ratios):\nsrc/invoice.py\n  3:from .money import to_money";
        assert_eq!(actual, expected);
        for line in fixture.lines() {
            let (_, rest) = line.split_once(':').unwrap();
            assert!(actual.contains(rest), "lost: {line}");
        }
    }

    #[test]
    fn test_output_not_in_match_shape_is_left_alone() {
        assert_eq!(regroup_search("no colon here\n"), None);
        assert_eq!(
            regroup_search("a.py:1:x\n"),
            None,
            "not smaller, so unchanged"
        );
    }
}
