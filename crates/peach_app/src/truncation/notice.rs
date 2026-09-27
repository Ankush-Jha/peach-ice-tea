/// Builds the standard, model-visible sentence for content a tool withheld
/// (R-OUT-4): how much was withheld, and the exact call to get it back.
///
/// Every truncation site in this crate renders through this function so the
/// wording is identical everywhere, and so the sentence always lands inside
/// the tool output's text/CDATA body rather than only in an XML attribute —
/// attributes are easy for a model to skim past; body text is not.
///
/// # Arguments
/// * `withheld_count` - how much content was left out, already counted in
///   whatever unit is being reported.
/// * `unit` - a plural, human-readable unit for `withheld_count`, e.g.
///   `"lines"`, `"chars"` or `"matches"`.
/// * `recovery_call` - the exact tool call that gets the withheld content back,
///   phrased as a complete sentence (e.g. `"Full output: read /tmp/x.txt (lines
///   100-1532)."`).
pub fn recovery_notice(withheld_count: u64, unit: &str, recovery_call: &str) -> String {
    format!("{withheld_count} more {unit} not shown. {recovery_call}")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_recovery_notice_format() {
        let actual = recovery_notice(
            3823,
            "lines",
            "Full output: read /tmp/peach-xyz.txt (lines 100-1532).",
        );
        let expected =
            "3823 more lines not shown. Full output: read /tmp/peach-xyz.txt (lines 100-1532).";

        assert_eq!(actual, expected);
    }
}
