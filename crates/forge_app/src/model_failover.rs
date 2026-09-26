//! Model failover (MM.4, D-072).
//!
//! A free-tier daily quota or a provider outage used to end the run on the
//! spot (D-040 made that fast, not survivable). With a fallback list, the run
//! continues on the next model of the same provider instead, and says so in a
//! `recovery` event. Without one, behaviour is unchanged (principle 5).

use forge_domain::{Error, ModelId};

/// Comma-separated model ids to fall back to, in order, on the session's
/// provider. Set per profile (`profile.env`); unset means no failover.
pub const ENV_VAR: &str = "FORGE_HARNESS_FALLBACK_MODELS";

/// Why `error` justifies trying another model, or `None` when it does not.
///
/// Two cases qualify: a quota that cannot clear within the run (D-040), and a
/// transient failure (rate limit, 5xx, transport) that survived every retry.
/// Anything else, such as a 400 for a malformed request, would fail the same
/// way on another model, so it still ends the run.
///
/// # Arguments
/// * `error` - The error the model call returned after its retries.
pub fn failover_reason(error: &anyhow::Error) -> Option<String> {
    if let Some(quota) = forge_domain::provider_quota::exhausted_quota(error) {
        return Some(format!("provider quota exhausted ({quota})"));
    }
    let retries_exhausted = error.chain().any(|cause| matches!(cause.downcast_ref::<Error>(), Some(Error::Retryable(_))));
    retries_exhausted.then(|| format!("still failing after retries: {}", error.root_cause()))
}

/// The models still to try, in order, never repeating one already used.
#[derive(Debug, Clone, PartialEq)]
pub struct Failover {
    remaining: Vec<ModelId>,
}

impl Failover {
    /// A failover list from `models`, without `current` or duplicates.
    ///
    /// # Arguments
    /// * `current` - The model the run starts on.
    /// * `models` - Comma-separated fallback ids.
    pub fn new(current: &ModelId, models: &str) -> Self {
        let mut remaining: Vec<ModelId> = Vec::new();
        for id in models.split(',').map(str::trim).filter(|id| !id.is_empty()) {
            let id = ModelId::new(id);
            if &id != current && !remaining.contains(&id) {
                remaining.push(id);
            }
        }
        Self { remaining }
    }

    /// The list from [`ENV_VAR`]; empty when it is unset.
    pub fn from_env(current: &ModelId) -> Self {
        Self::new(current, &std::env::var(ENV_VAR).unwrap_or_default())
    }

    /// The next model to try, removed from the list.
    pub fn next_model(&mut self) -> Option<ModelId> {
        (!self.remaining.is_empty()).then(|| self.remaining.remove(0))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_the_list_skips_the_current_model_blanks_and_repeats() {
        let mut fixture = Failover::new(&ModelId::new("a"), " b, a,,c , b");

        let actual = [fixture.next_model(), fixture.next_model(), fixture.next_model()];

        assert_eq!(actual, [Some(ModelId::new("b")), Some(ModelId::new("c")), None]);
    }

    #[test]
    fn test_only_unrecoverable_quotas_and_exhausted_retries_fail_over() {
        let quota = anyhow::anyhow!("Invalid Status Code: 402")
            .context(r#"402 Reason: {"error":{"metadata":{"limit_source":"openrouter_credits"}}}"#);
        let retried: anyhow::Error = Error::Retryable(anyhow::anyhow!("Invalid Status Code: 503")).into();
        let bad_request = anyhow::anyhow!("Invalid Status Code: 400");

        let actual = [&quota, &retried, &bad_request].map(|e| failover_reason(e).is_some());

        assert_eq!(actual, [true, true, false]);
    }
}
