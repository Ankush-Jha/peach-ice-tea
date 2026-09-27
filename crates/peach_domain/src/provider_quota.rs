//! harness: recognising a provider quota that cannot recover within a run.
//!
//! D-032a: a free-tier Gemini key hit its per-day request quota, and peach
//! spent 5.5 minutes on 8 retries that could never succeed, while the only
//! line saying why (the 429 body) was dropped from the evidence. A per-minute
//! rate limit is worth retrying; a per-day or billing quota is not.

/// The identifier of an exhausted quota that retrying within this run cannot
/// clear, found anywhere in `error`'s context chain (providers attach the
/// response body there). Only the quota identifier is returned, never the
/// body, which can carry account details.
///
/// Recognised:
/// - Google: a `quotaId` naming a per-day quota (`...PerDay...`);
/// - OpenAI-style: `insufficient_quota` (no credit left);
/// - OpenRouter: a 402 whose `limit_source` is `openrouter_credits` (the
///   account cannot afford the request's `max_tokens`; D-050).
pub fn exhausted_quota(error: &anyhow::Error) -> Option<String> {
    let text = format!("{error:#}");
    if text.contains("insufficient_quota") {
        return Some("insufficient_quota".to_string());
    }
    if text
        .replace("\\\"", "\"")
        .contains("\"limit_source\":\"openrouter_credits\"")
    {
        return Some("openrouter_credits".to_string());
    }
    quota_ids(&text)
        .into_iter()
        .find(|id| id.contains("PerDay"))
}

/// Every `"quotaId": "<id>"` value in `text`, with or without a space after
/// the colon and with escaped quotes (the body may be JSON inside JSON).
fn quota_ids(text: &str) -> Vec<String> {
    let unescaped = text.replace("\\\"", "\"");
    // Each piece after the first follows a `"quotaId"` key. Splitting (rather
    // than slicing at byte offsets) cannot panic on multi-byte text.
    unescaped
        .split("\"quotaId\"")
        .skip(1)
        .filter_map(|rest| {
            let value = rest
                .trim_start()
                .strip_prefix(':')?
                .trim_start()
                .strip_prefix('"')?;
            let (id, _) = value.split_once('"')?;
            Some(id.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const GOOGLE_DAILY: &str = r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","details":[{"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"quotaMetric":"generativelanguage.googleapis.com/generate_content_free_tier_requests","quotaId":"GenerateRequestsPerDayPerProjectPerModel-FreeTier","quotaValue":"20"}]},{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"59s"}]}}"#;
    const GOOGLE_MINUTE: &str = r#"{"error":{"code":429,"details":[{"violations":[{"quotaId": "GenerateRequestsPerMinutePerProjectPerModel"}]}]}}"#;

    const OPENROUTER_CREDITS: &str = r#"{"error":{"message":"This request requires more credits, or fewer max_tokens. You requested up to 20480 tokens, but can only afford 1354.","code":402,"metadata":{"limit_source":"openrouter_credits"}}}"#;

    fn provider_error(body: &str) -> anyhow::Error {
        anyhow::anyhow!("Invalid Status Code: 429")
            .context(format!("429 Too Many Requests Reason: {body}"))
            .context("POST https://generativelanguage.googleapis.com/v1beta/models/x:streamGenerateContent")
    }

    #[test]
    fn test_only_quotas_that_cannot_recover_in_the_run_are_reported() {
        let actual = vec![
            exhausted_quota(&provider_error(GOOGLE_DAILY)),
            exhausted_quota(&provider_error(GOOGLE_MINUTE)),
            exhausted_quota(&provider_error(
                r#"{"error":{"code":"insufficient_quota"}}"#,
            )),
            exhausted_quota(&anyhow::anyhow!("Invalid Status Code: 503")),
            exhausted_quota(&provider_error(&GOOGLE_DAILY.replace('"', "\\\""))),
            exhausted_quota(&provider_error(OPENROUTER_CREDITS)),
        ];

        let expected = vec![
            Some("GenerateRequestsPerDayPerProjectPerModel-FreeTier".to_string()),
            None,
            Some("insufficient_quota".to_string()),
            None,
            Some("GenerateRequestsPerDayPerProjectPerModel-FreeTier".to_string()),
            Some("openrouter_credits".to_string()),
        ];
        assert_eq!(actual, expected);
    }
}
