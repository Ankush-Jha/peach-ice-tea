//! What a test run's result means (R-HACK-7): pass/fail/skip counts parsed
//! from the runner's own summary, and a failure class for recovery and the
//! report. Counts the harness cannot parse stay `None` — never a guess.

use serde::{Deserialize, Serialize};

/// Kind of test-run result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// Exit code 0.
    Passed,
    /// Tests ran and at least one assertion failed.
    TestAssertion,
    /// The code did not compile, or failed to import/parse.
    Compile,
    /// The toolchain, a dependency or the runner itself is missing.
    Environment,
    /// The run was stopped for taking too long.
    Timeout,
    /// Failed for a reason the harness does not recognise.
    Unknown,
}

impl FailureClass {
    /// The snake_case name used in telemetry and evidence.
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureClass::Passed => "passed",
            FailureClass::TestAssertion => "test_assertion",
            FailureClass::Compile => "compile",
            FailureClass::Environment => "environment",
            FailureClass::Timeout => "timeout",
            FailureClass::Unknown => "unknown",
        }
    }
}

/// A classified test run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Classification {
    pub class: FailureClass,
    pub passed: Option<u64>,
    pub failed: Option<u64>,
    pub skipped: Option<u64>,
}

/// Classifies a finished test command from its exit code and output.
///
/// # Arguments
/// * `exit_code` - `None` when the process did not exit on its own.
/// * `timed_out` - Whether the harness stopped it for time.
/// * `output` - stdout and stderr together.
pub fn classify(exit_code: Option<i32>, timed_out: bool, output: &str) -> Classification {
    let (passed, failed, skipped) = counts(output);
    let class = if timed_out {
        FailureClass::Timeout
    } else if exit_code == Some(0) {
        FailureClass::Passed
    } else if ENVIRONMENT.iter().any(|marker| output.contains(marker)) {
        FailureClass::Environment
    } else if COMPILE.iter().any(|marker| output.contains(marker)) && passed.unwrap_or(0) == 0 {
        // A compile/import marker with nothing passing: the code did not
        // load. With some tests passing, it is one broken module among
        // working ones, so it falls through to an assertion failure.
        FailureClass::Compile
    } else if failed.is_some_and(|f| f > 0) || ASSERTION.iter().any(|marker| output.contains(marker)) {
        FailureClass::TestAssertion
    } else {
        FailureClass::Unknown
    };
    Classification { class, passed, failed, skipped }
}

const ENVIRONMENT: &[&str] = &[
    "No module named pytest",
    "command not found",
    "Cannot find module",
    "not recognized as an internal or external command",
    "error: no such command",
    "is not installed",
    "No such file or directory (os error 2)",
];

const COMPILE: &[&str] = &[
    "error[E",
    "could not compile",
    "SyntaxError",
    "IndentationError",
    "ImportError",
    "ModuleNotFoundError",
    "TypeError: Cannot read",
    "error TS",
    "cannot find symbol",
    "undefined: ",
];

const ASSERTION: &[&str] = &["AssertionError", "assertion failed", "--- FAIL", "FAILED", "not ok "];

/// `(passed, failed, skipped)` from the first runner summary format found.
fn counts(output: &str) -> (Option<u64>, Option<u64>, Option<u64>) {
    // cargo: `test result: ok. 3 passed; 1 failed; 0 ignored; ...` per binary.
    let cargo: Vec<&str> = output.lines().filter(|l| l.contains("test result:")).collect();
    if !cargo.is_empty() {
        let sum = |word: &str| cargo.iter().filter_map(|l| number_before(l, word)).sum::<u64>();
        return (Some(sum(" passed")), Some(sum(" failed")), Some(sum(" ignored")));
    }
    // unittest: `Ran 5 tests` then `OK` or `FAILED (failures=1, errors=1, skipped=2)`.
    if let Some(ran) = output.lines().find_map(|l| l.strip_prefix("Ran ").and_then(first_number)) {
        let field = |name: &str| {
            output
                .lines()
                .rev()
                .find(|l| l.starts_with("OK") || l.starts_with("FAILED"))
                .and_then(|l| number_after(l, &format!("{name}=")))
                .unwrap_or(0)
        };
        let failed = field("failures") + field("errors");
        let skipped = field("skipped");
        return (Some(ran.saturating_sub(failed + skipped)), Some(failed), Some(skipped));
    }
    // node --test (TAP): `# pass 3`, `# fail 1`, `# skipped 0`.
    if output.contains("# pass ") {
        let tap = |name: &str| output.lines().find_map(|l| l.strip_prefix(name).and_then(first_number));
        return (tap("# pass "), tap("# fail "), tap("# skipped ").or(Some(0)));
    }
    // jest/vitest: `Tests:       1 failed, 4 passed, 5 total`.
    if let Some(line) = output.lines().find(|l| l.trim_start().starts_with("Tests:")) {
        return (
            number_before(line, " passed").or(Some(0)),
            number_before(line, " failed").or(Some(0)),
            number_before(line, " skipped").or(Some(0)),
        );
    }
    // pytest: `==== 1 failed, 4 passed, 1 skipped in 0.12s ====`.
    if let Some(line) = output
        .lines()
        .rev()
        .find(|l| l.contains(" passed") || l.contains(" failed") || l.contains(" error"))
        .filter(|l| l.contains(" in ") && l.contains('s'))
    {
        let failed = number_before(line, " failed").unwrap_or(0) + number_before(line, " error").unwrap_or(0);
        return (
            number_before(line, " passed").or(Some(0)),
            Some(failed),
            number_before(line, " skipped").or(Some(0)),
        );
    }
    // go: one `--- PASS:` / `--- FAIL:` / `--- SKIP:` per test with -v.
    if output.contains("--- PASS:") || output.contains("--- FAIL:") {
        let count = |marker: &str| output.matches(marker).count() as u64;
        return (Some(count("--- PASS:")), Some(count("--- FAIL:")), Some(count("--- SKIP:")));
    }
    (None, None, None)
}

fn first_number(text: &str) -> Option<u64> {
    text.split(|c: char| !c.is_ascii_digit()).find(|part| !part.is_empty())?.parse().ok()
}

/// The number immediately before `word` in `line` (`"3 passed"` → 3).
fn number_before(line: &str, word: &str) -> Option<u64> {
    let index = line.find(word)?;
    line.split_at(index).0.split(|c: char| !c.is_ascii_digit()).rev().find(|p| !p.is_empty())?.parse().ok()
}

/// The number immediately after `prefix` in `line` (`"failures=2"` → 2).
fn number_after(line: &str, prefix: &str) -> Option<u64> {
    let index = line.find(prefix)? + prefix.len();
    first_number(line.get(index..)?)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture(class: FailureClass, counts: (Option<u64>, Option<u64>, Option<u64>)) -> Classification {
        Classification { class, passed: counts.0, failed: counts.1, skipped: counts.2 }
    }

    #[test]
    fn test_real_runner_outputs_are_classified_and_counted() {
        let unittest_fail = "test_a (tests.test_stats.T) ... FAIL\n\nFAIL: test_a\nAssertionError: Lists differ\n\n----\nRan 3 tests in 0.001s\n\nFAILED (failures=1, skipped=1)\n";
        let unittest_ok = "...\n----------------------------------------------------------------------\nRan 3 tests in 0.000s\n\nOK\n";
        let pytest = "tests/test_a.py .F.s\n=========== 1 failed, 2 passed, 1 skipped in 0.12s ===========\n";
        let cargo_compile = "error[E0308]: mismatched types\nerror: could not compile `x` due to 1 previous error\n";
        let cargo_mixed = "test result: ok. 3 passed; 0 failed; 1 ignored\ntest result: FAILED. 1 passed; 2 failed; 0 ignored\n";
        let node = "not ok 1 - stub\n# tests 3\n# pass 2\n# fail 1\n# skipped 0\n";
        let jest = "Tests:       1 failed, 4 passed, 5 total\n";
        let missing = "/usr/bin/python3: No module named pytest\n";
        let import = "ImportError: cannot import name 'dedupe' from 'stats'\nRan 0 tests\n\nFAILED (errors=1)\n";

        let actual = vec![
            classify(Some(1), false, unittest_fail),
            classify(Some(0), false, unittest_ok),
            classify(Some(1), false, pytest),
            classify(Some(101), false, cargo_compile),
            classify(Some(101), false, cargo_mixed),
            classify(Some(1), false, node),
            classify(Some(1), false, jest),
            classify(Some(1), false, missing),
            classify(Some(1), false, import),
            classify(None, true, ""),
            classify(Some(2), false, "something odd"),
        ];

        let expected = vec![
            fixture(FailureClass::TestAssertion, (Some(1), Some(1), Some(1))),
            fixture(FailureClass::Passed, (Some(3), Some(0), Some(0))),
            fixture(FailureClass::TestAssertion, (Some(2), Some(1), Some(1))),
            fixture(FailureClass::Compile, (None, None, None)),
            fixture(FailureClass::TestAssertion, (Some(4), Some(2), Some(1))),
            fixture(FailureClass::TestAssertion, (Some(2), Some(1), Some(0))),
            fixture(FailureClass::TestAssertion, (Some(4), Some(1), Some(0))),
            fixture(FailureClass::Environment, (None, None, None)),
            fixture(FailureClass::Compile, (Some(0), Some(1), Some(0))),
            fixture(FailureClass::Timeout, (None, None, None)),
            fixture(FailureClass::Unknown, (None, None, None)),
        ];
        assert_eq!(actual, expected);
    }
}
