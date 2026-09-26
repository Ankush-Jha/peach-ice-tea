//! Who this harness is, for the evidence record.
//!
//! The hackathon's evaluation record carries a harness name and version
//! (`HACKATHON.md` §30), and telemetry, the evidence manifest and the report
//! all have to agree on them. This module is the single place the name is
//! written down (`DECISIONS.md` D-023); nothing else should contain the
//! literal.

use serde::{Deserialize, Serialize};

/// Machine-readable harness name, used in telemetry and reports.
pub const HARNESS_NAME: &str = "peach-ice-tea";

/// Human-readable harness name, used in documentation and banners.
pub const HARNESS_DISPLAY_NAME: &str = "Peach Ice Tea";

/// Harness version, taken from the crate version so it cannot drift.
pub const HARNESS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version of the internal telemetry event schema.
///
/// Independent of `HARNESS_VERSION`: the harness can change without the event
/// contract changing, and a consumer cares about the contract.
pub const TELEMETRY_SCHEMA_VERSION: &str = "0.1.0";

/// Version of the internal report schema.
pub const REPORT_SCHEMA_VERSION: &str = "0.1.0";

/// Identifies the harness that produced a piece of evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessIdentity {
    /// Machine-readable name.
    pub name: String,
    /// Harness version.
    pub version: String,
    /// Build identifier, when one was supplied at compile time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
}

impl HarnessIdentity {
    /// Identity of the running harness.
    pub fn current() -> Self {
        Self {
            name: HARNESS_NAME.to_string(),
            version: HARNESS_VERSION.to_string(),
            build: option_env!("PEACH_ICE_TEA_BUILD").map(str::to_string),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_current_identity_reports_the_crate_version() {
        let actual = HarnessIdentity::current();

        assert_eq!(actual.name, "peach-ice-tea");
        assert_eq!(actual.version, env!("CARGO_PKG_VERSION"));
    }
}
