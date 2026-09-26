//! Harness-specific subsystems for Peach Ice Tea.
//!
//! Everything here is additive to the upstream ForgeCode crates: upstream code
//! calls into this crate, never the other way round, so merges stay cheap
//! (`DECISIONS.md` D-004). The crate deliberately does not depend on
//! `forge_app`, to keep the dependency graph acyclic.

pub mod identity;
pub mod evidence;
pub mod integrity;
pub mod redact;
pub mod report;
pub mod runtime;
pub mod scorer;
pub mod telemetry;
