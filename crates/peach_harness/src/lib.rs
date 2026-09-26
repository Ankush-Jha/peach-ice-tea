//! Harness-specific subsystems for Peach Ice Tea.
//!
//! Everything here is additive to the upstream Peach Ice Tea crates: upstream code
//! calls into this crate, never the other way round, so merges stay cheap
//! (`DECISIONS.md` D-004). The crate deliberately does not depend on
//! `peach_app`, to keep the dependency graph acyclic.

pub mod identity;
pub mod integrity;
pub mod runtime;
