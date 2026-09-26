//! Noise compression for build, install and test output (R-OUT-2, T1.5).
//!
//! A 3,000-line `cargo build` or `npm install` spends the model's context on
//! progress bars and "Compiling x" lines; the line that matters is the one
//! error. This keeps every line that could matter and collapses the rest,
//! loudly, with the full output one `read` away. Behind
//! `PEACH_HARNESS_NOISE_COMPRESSION=1`, default off until an A/B, which must
//! also show the noise class's recovery rate under 2% (R-OUT-2, D-073).

/// Environment variable that turns noise compression on.
pub const NOISE_COMPRESSION_ENV_VAR: &str = "PEACH_HARNESS_NOISE_COMPRESSION";

/// Whether noise compression is enabled for this process.
pub fn noise_compression_enabled() -> bool {
    std::env::var(NOISE_COMPRESSION_ENV_VAR).as_deref() == Ok("1")
}

/// Words that make a line worth keeping verbatim, with its context.
const SIGNAL: &[&str] = &["error", "warn", "fail", "panic", "exception", "traceback", "assert", "fatal", "denied", "not found"];
/// Lines kept around each signal line, before and after.
const CONTEXT: usize = 3;
/// Trailing lines always kept: where tools print their summary.
const SUMMARY_LINES: usize = 15;
/// Runs of at least this many similar lines are collapsed.
const MIN_RUN: usize = 3;
/// Compress only when it saves at least this fraction and this many chars.
const MIN_SAVING_RATIO: f64 = 0.30;
const MIN_SAVING_CHARS: usize = 2_000;

/// The compressed output and what was collapsed, or `None` when compressing
/// would not save enough to be worth a lossy view (the output then passes
/// through unchanged).
///
/// # Arguments
/// * `raw` - The command's output (stdout or stderr).
pub fn compress_noise(raw: &str) -> Option<(String, usize)> {
    let lines: Vec<String> = raw.lines().map(clean_line).collect();
    let keep = keep_mask(&lines);

    let mut out: Vec<String> = Vec::new();
    let mut collapsed = 0usize;
    let mut i = 0;
    while i < lines.len() {
        if keep[i] {
            out.push(lines[i].clone());
            i += 1;
            continue;
        }
        // A stretch of lines nothing marked as important.
        let start = i;
        while i < lines.len() && !keep[i] {
            i += 1;
        }
        let (passing, rest): (Vec<&String>, Vec<&String>) = lines[start..i].iter().partition(|l| is_passing_test(l));
        if !passing.is_empty() {
            out.push(format!("… {} passing test lines", passing.len()));
            collapsed += passing.len();
        }
        let mut j = 0;
        while j < rest.len() {
            let shape = normalise(rest[j]);
            let run = rest[j..].iter().take_while(|l| normalise(l) == shape).count();
            if run >= MIN_RUN {
                out.push(rest[j].clone());
                out.push(format!("… {} similar lines", run - 1));
                collapsed += run - 1;
            } else {
                out.extend(rest[j..j + run].iter().map(|l| (*l).clone()));
            }
            j += run;
        }
    }

    let compressed = out.join("\n");
    let saved = raw.len().saturating_sub(compressed.len());
    (saved >= MIN_SAVING_CHARS && saved as f64 >= raw.len() as f64 * MIN_SAVING_RATIO && collapsed > 0)
        .then_some((compressed, collapsed))
}

/// Strips ANSI escapes and keeps only the last carriage-return frame of a
/// progress line.
fn clean_line(line: &str) -> String {
    let frame = line.rsplit('\r').next().unwrap_or(line);
    let mut out = String::with_capacity(frame.len());
    let mut chars = frame.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn keep_mask(lines: &[String]) -> Vec<bool> {
    let mut keep = vec![false; lines.len()];
    for (i, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();
        if SIGNAL.iter().any(|word| lower.contains(word)) && !is_passing_test(line) {
            let from = i.saturating_sub(CONTEXT);
            let to = (i + CONTEXT + 1).min(lines.len());
            keep[from..to].iter_mut().for_each(|k| *k = true);
        }
    }
    let summary_from = lines.len().saturating_sub(SUMMARY_LINES);
    keep[summary_from..].iter_mut().for_each(|k| *k = true);
    keep
}

/// A line reporting one passing test, in the shapes cargo, pytest, jest,
/// go and unittest print.
fn is_passing_test(line: &str) -> bool {
    let t = line.trim();
    t.ends_with(" ... ok")
        || t.ends_with(" PASSED")
        || t.starts_with("PASSED ")
        || t.starts_with("✓ ")
        || t.starts_with("√ ")
        || t.starts_with("--- PASS:")
        || t.starts_with("ok ") && t.contains("test")
}

/// A line with its digits and hex-ish tokens blurred, so `Compiling foo
/// v1.2.3` and `Compiling bar v0.9.0` count as similar.
fn normalise(line: &str) -> String {
    line.split_whitespace()
        .enumerate()
        .map(|(i, word)| if i == 0 { word.to_string() } else if word.chars().any(|c| c.is_ascii_digit()) { "#".into() } else { "*".into() })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn cargo_build_with_one_error() -> String {
        let mut log: Vec<String> = (0..300).map(|i| format!("   Compiling crate{i} v0.{i}.1")).collect();
        log.push("error[E0308]: mismatched types".into());
        log.push("  --> src/money.rs:14:5".into());
        log.push("   |".into());
        log.push("14 |     amount * ratio".into());
        log.push("   |     ^^^^^^^^^^^^^^ expected `Decimal`, found `f64`".into());
        log.push("error: could not compile `ledger` (lib) due to 1 previous error".into());
        log.join("\n")
    }

    #[test]
    fn test_a_cargo_build_keeps_the_error_and_collapses_the_compiling_lines() {
        let fixture = cargo_build_with_one_error();

        let (actual, collapsed) = compress_noise(&fixture).unwrap();

        assert!(actual.contains("error[E0308]: mismatched types"));
        assert!(actual.contains("expected `Decimal`, found `f64`"));
        assert!(actual.contains("could not compile `ledger`"));
        assert!(actual.contains("similar lines"));
        assert!(collapsed > 250, "collapsed {collapsed}");
        assert!(actual.len() * 5 < fixture.len(), "{} vs {}", actual.len(), fixture.len());
    }

    #[test]
    fn test_pytest_passes_are_counted_and_the_failure_kept_with_context() {
        let mut log: Vec<String> = (0..200).map(|i| format!("tests/test_money.py::test_case_{i} PASSED")).collect();
        log.insert(120, "tests/test_money.py::test_half_cent FAILED".into());
        log.push("E       AssertionError: assert Decimal('2.66') == Decimal('2.67')".into());
        log.push("=========== 1 failed, 200 passed in 0.52s ===========".into());
        let fixture = log.join("\n");

        let (actual, _) = compress_noise(&fixture).unwrap();

        assert!(actual.contains("test_half_cent FAILED"));
        assert!(actual.contains("AssertionError: assert Decimal('2.66') == Decimal('2.67')"));
        assert!(actual.contains("1 failed, 200 passed"));
        assert!(actual.contains("passing test lines"));
    }

    #[test]
    fn test_npm_progress_frames_and_ansi_are_stripped() {
        let frames: String = (0..400).map(|i| format!("\u{1b}[32m⸨{i:>3}%⸩\u{1b}[0m reify:package-{i}: timing reifyNode\r")).collect();
        let fixture = format!("{frames}\nadded 812 packages in 9s\nnpm WARN deprecated left-pad@1.3.0: use String.padStart\n");

        let (actual, _) = compress_noise(&fixture).unwrap_or((clean_line(&fixture), 0));

        assert!(!actual.contains('\u{1b}'));
        assert!(actual.contains("npm WARN deprecated left-pad"));
        assert!(actual.contains("added 812 packages"));
    }

    #[test]
    fn test_short_or_signal_dense_output_passes_through() {
        let short = "error: one\nerror: two\n";
        let dense: String = (0..200).map(|i| format!("error: problem {i}\n")).collect();

        assert_eq!(compress_noise(short), None);
        assert_eq!(compress_noise(&dense), None, "every line is a signal line; nothing to collapse");
    }
}
