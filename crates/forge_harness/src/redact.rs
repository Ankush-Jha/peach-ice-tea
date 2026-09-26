//! Secret redaction before anything leaves the process (`R-SAFE-3`).
//!
//! Every tool-call preview that might reach a relevance scorer, an external
//! hook, or a log line passes through here first (`DECISIONS.md` D-027,
//! S2's "secrets redacted from input previews sent to the scorer"). The scan
//! is plain string matching over bytes — no regex crate is available to this
//! crate — and is written to do one linear pass per pattern class so it stays
//! cheap to run on every preview.
//!
//! Two complementary strategies are used:
//! - **Key-shaped matches**: an identifier that looks like a secret-bearing
//!   key (`api_key`, `token`, `password`, ...) immediately followed by a
//!   separator (`=`, `:`, `": "`) redacts the value that follows, whatever it
//!   looks like.
//! - **Value-shaped matches**: a handful of well-known credential formats
//!   (AWS, Google, OpenAI-style, GitHub, JWT, bearer tokens) are recognisable
//!   from their own shape and are redacted wherever they appear, even with no
//!   key name nearby.
//!
//! Both strategies favour precision over recall in one place and recall over
//! precision in the other, on purpose: a key-shaped match only fires on a
//! short, explicit allow-list of key substrings, so it is safe to redact
//! whatever value follows even if that value is plain code; a value-shaped
//! match has no key context to lean on, so each pattern requires a
//! non-alphanumeric character immediately before it (a word-boundary guard
//! beyond the literal regexes this ports) to avoid firing inside an unrelated
//! longer identifier, and — for the bearer-token pattern specifically — a
//! minimum length plus at least one digit or symbol, to avoid firing on an
//! ordinary long English word that happens to follow the word "bearer" in
//! prose.

use std::borrow::Cow;

const REDACTED: &str = "[REDACTED]";

/// Key substrings (already lower-cased, `-` normalised to `_`) that mark a
/// key/value pair as secret-bearing. Matching is substring containment, so
/// e.g. `x-api-key` and `apiKeyForUser` both match `api_key`.
const SENSITIVE_KEY_SUBSTRINGS: &[&str] = &[
    "api_key",
    "apikey",
    "token",
    "password",
    "passwd",
    "secret",
    "authorization",
    "bearer",
    "credential",
    "private_key",
];

/// Redacts secret-looking values in `input`, replacing them with
/// `[REDACTED]`.
///
/// Borrows `input` unchanged when nothing matches, so the common case (the
/// vast majority of tool output) allocates nothing.
pub fn redact(input: &str) -> Cow<'_, str> {
    let spans = merge_spans(find_spans(input));
    if spans.is_empty() {
        return Cow::Borrowed(input);
    }

    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;
    for (start, end) in spans {
        output.push_str(&input[cursor..start]);
        output.push_str(REDACTED);
        cursor = end;
    }
    output.push_str(&input[cursor..]);
    Cow::Owned(output)
}

/// Redacts values keyed by a secret-bearing field name anywhere in a JSON
/// tree, in place.
///
/// Only the key name decides whether a value is redacted; the value itself
/// is replaced with a JSON string `"[REDACTED]"` regardless of its original
/// type. Nested objects and arrays are walked recursively so a secret buried
/// several levels deep is still caught.
pub fn redact_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, entry) in map.iter_mut() {
                if is_sensitive_key(key) {
                    *entry = serde_json::Value::String(REDACTED.to_string());
                } else {
                    redact_json(entry);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                redact_json(item);
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace('-', "_");
    SENSITIVE_KEY_SUBSTRINGS.iter().any(|needle| normalized.contains(needle))
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

fn is_gap_byte(byte: u8) -> bool {
    byte == b'"' || byte == b'\'' || byte.is_ascii_whitespace()
}

fn preceded_by_word_char(bytes: &[u8], index: usize) -> bool {
    index > 0 && bytes[index - 1].is_ascii_alphanumeric()
}

/// Finds every byte span that should be replaced with `[REDACTED]`, in the
/// order found. Spans may overlap; `merge_spans` reconciles that.
fn find_spans(input: &str) -> Vec<(usize, usize)> {
    let mut spans = find_key_value_spans(input);
    find_value_shape_spans(input, &mut spans);
    spans
}

/// Pass 1: `key = value`, `key: value` and `"key": "value"` shapes, where
/// `key` contains one of `SENSITIVE_KEY_SUBSTRINGS`.
fn find_key_value_spans(input: &str) -> Vec<(usize, usize)> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut spans = Vec::new();

    let mut word_start: Option<usize> = None;
    let mut last_word: Option<(usize, usize)> = None;
    let mut i = 0usize;
    while i < len {
        let byte = bytes[i];

        if is_ident_byte(byte) {
            if word_start.is_none() {
                word_start = Some(i);
            }
            i += 1;
            continue;
        }

        if let Some(start) = word_start.take() {
            last_word = Some((start, i));
        }

        if byte == b'=' || byte == b':' {
            let sensitive_value = last_word
                .filter(|&(word_from, word_to)| is_sensitive_key(&input[word_from..word_to]))
                .and_then(|_| parse_value(bytes, i + 1));
            if let Some((value_start, value_end, next)) = sensitive_value {
                if value_start < value_end {
                    spans.push((value_start, value_end));
                }
                last_word = None;
                i = next;
                continue;
            }
            last_word = None;
            i += 1;
            continue;
        }

        if is_gap_byte(byte) {
            // Whitespace and quote characters between a key and its
            // separator don't invalidate the key (`"api_key" : "x"`,
            // `token = x`), so keep `last_word` alive across them.
            i += 1;
            continue;
        }

        last_word = None;
        i += 1;
    }

    spans
}

/// Parses the value following a `=`/`:` separator, starting at `start`.
///
/// Returns `(value_start, value_end, next_index)`: the redactable span
/// (excluding surrounding quotes, if any) and the index scanning should
/// resume from. Leading spaces/tabs are skipped. A quoted value ends at its
/// matching (unescaped) closing quote; an unquoted value ends at whitespace
/// or a small set of separators (`&`, `;`, `,`, `}`, `)`, `]`) common to
/// query strings, shell assignments and structured text — deliberately not
/// consuming across a space, so an inline shell assignment like
/// `API_KEY=x command --flag` doesn't swallow the rest of the line.
fn parse_value(bytes: &[u8], mut i: usize) -> Option<(usize, usize, usize)> {
    let len = bytes.len();
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= len {
        return Some((i, i, i));
    }

    if bytes[i] == b'"' || bytes[i] == b'\'' {
        let quote = bytes[i];
        let start = i + 1;
        let mut j = start;
        while j < len {
            if bytes[j] == b'\\' && j + 1 < len {
                j += 2;
                continue;
            }
            if bytes[j] == quote {
                break;
            }
            j += 1;
        }
        let end = j.min(len);
        let next = if j < len { j + 1 } else { j };
        return Some((start, end, next));
    }

    let start = i;
    while i < len {
        let byte = bytes[i];
        if byte.is_ascii_whitespace() || matches!(byte, b'&' | b';' | b',' | b'}' | b')' | b']') {
            break;
        }
        i += 1;
    }
    Some((start, i, i))
}

/// Pass 2: recognisable secret shapes, independent of any surrounding key.
fn find_value_shape_spans(input: &str, spans: &mut Vec<(usize, usize)>) {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    while i < len {
        if let Some(end) = match_aws_key(bytes, i)
            .or_else(|| match_google_key(bytes, i))
            .or_else(|| match_openai_key(bytes, i))
            .or_else(|| match_github_token(bytes, i))
            .or_else(|| match_jwt(bytes, i))
            .or_else(|| match_bearer_token(bytes, i))
        {
            spans.push((i, end));
            i = end;
            continue;
        }
        i += 1;
    }
}

/// AWS access key: `AKIA` followed by exactly 16 `[0-9A-Z]` characters.
fn match_aws_key(bytes: &[u8], i: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"AKIA";
    if preceded_by_word_char(bytes, i) || !bytes[i..].starts_with(PREFIX) {
        return None;
    }
    let start = i + PREFIX.len();
    let end = start.checked_add(16)?;
    if end > bytes.len() {
        return None;
    }
    if bytes[start..end].iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
        Some(end)
    } else {
        None
    }
}

/// Google API key: `AIza` followed by exactly 35 `[0-9A-Za-z_-]` characters.
fn match_google_key(bytes: &[u8], i: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"AIza";
    if preceded_by_word_char(bytes, i) || !bytes[i..].starts_with(PREFIX) {
        return None;
    }
    let start = i + PREFIX.len();
    let end = start.checked_add(35)?;
    if end > bytes.len() {
        return None;
    }
    if bytes[start..end].iter().all(|&b| is_ident_byte(b)) {
        Some(end)
    } else {
        None
    }
}

/// OpenAI-style key: `sk-` followed by 20 or more `[A-Za-z0-9_-]` characters.
fn match_openai_key(bytes: &[u8], i: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"sk-";
    if preceded_by_word_char(bytes, i) || !bytes[i..].starts_with(PREFIX) {
        return None;
    }
    let start = i + PREFIX.len();
    let mut end = start;
    while end < bytes.len() && is_ident_byte(bytes[end]) {
        end += 1;
    }
    if end - start >= 20 { Some(end) } else { None }
}

/// GitHub token: `gh` + one of `p`/`o`/`u`/`s`/`r` + `_` + 36 or more
/// alphanumeric characters.
fn match_github_token(bytes: &[u8], i: usize) -> Option<usize> {
    if preceded_by_word_char(bytes, i) || !bytes[i..].starts_with(b"gh") {
        return None;
    }
    let type_byte = *bytes.get(i + 2)?;
    if !matches!(type_byte, b'p' | b'o' | b'u' | b's' | b'r') {
        return None;
    }
    if bytes.get(i + 3) != Some(&b'_') {
        return None;
    }
    let start = i + 4;
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_alphanumeric() {
        end += 1;
    }
    if end - start >= 36 { Some(end) } else { None }
}

fn is_base64url_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'=')
}

/// Minimum length of each JWT segment. Real JWT headers/payloads/signatures
/// are comfortably longer than this even for minimal claims; the floor
/// exists to reject unrelated `word.word.word` text that happens to start
/// with `eyJ`.
const MIN_JWT_SEGMENT_LEN: usize = 16;

/// JWT: `eyJ` (the base64url-encoded start of a JSON header) followed by two
/// more `.`-separated base64url segments (payload, signature).
fn match_jwt(bytes: &[u8], i: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"eyJ";
    if preceded_by_word_char(bytes, i) || !bytes[i..].starts_with(PREFIX) {
        return None;
    }
    let len = bytes.len();
    let mut j = i;
    while j < len && is_base64url_byte(bytes[j]) {
        j += 1;
    }
    if j - i < MIN_JWT_SEGMENT_LEN || j >= len || bytes[j] != b'.' {
        return None;
    }
    j += 1;
    let segment_two_start = j;
    while j < len && is_base64url_byte(bytes[j]) {
        j += 1;
    }
    if j - segment_two_start < MIN_JWT_SEGMENT_LEN || j >= len || bytes[j] != b'.' {
        return None;
    }
    j += 1;
    let segment_three_start = j;
    while j < len && is_base64url_byte(bytes[j]) {
        j += 1;
    }
    if j - segment_three_start < MIN_JWT_SEGMENT_LEN {
        return None;
    }
    Some(j)
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+' | b'/' | b'=')
}

/// `Bearer <token>` (case-insensitive), covering `Authorization: Bearer …`
/// headers even though the value continues past the space that would
/// otherwise stop the key/value scan. Requires the token to be at least 16
/// characters and to contain a digit or symbol, so an ordinary sentence
/// ("the bond bearer receives interest") doesn't get treated as a header.
fn match_bearer_token(bytes: &[u8], i: usize) -> Option<usize> {
    const WORD: &[u8] = b"bearer";
    let len = bytes.len();
    if bytes[i] != b'B' && bytes[i] != b'b' {
        return None;
    }
    if preceded_by_word_char(bytes, i) || i + WORD.len() > len {
        return None;
    }
    for (offset, expected) in WORD.iter().enumerate() {
        if !bytes[i + offset].eq_ignore_ascii_case(expected) {
            return None;
        }
    }

    let mut j = i + WORD.len();
    if j >= len || !bytes[j].is_ascii_whitespace() {
        return None;
    }
    while j < len && bytes[j].is_ascii_whitespace() {
        j += 1;
    }

    let token_start = j;
    while j < len && is_token_byte(bytes[j]) {
        j += 1;
    }
    if j - token_start < 16 {
        return None;
    }
    let token = &bytes[token_start..j];
    let looks_like_a_token =
        token.iter().any(|b| b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.' | b'+' | b'/' | b'='));
    if !looks_like_a_token {
        return None;
    }

    Some(j)
}

/// Sorts and unions overlapping or touching spans so overlapping matches
/// (e.g. a key/value match and a value-shape match landing on the same
/// text) don't produce nested or duplicated `[REDACTED]` markers.
fn merge_spans(mut spans: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if spans.is_empty() {
        return spans;
    }
    spans.sort_unstable_by_key(|span| span.0);

    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(spans.len());
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_plain_text_is_returned_borrowed_and_unchanged() {
        let fixture = "The quick brown fox jumps over the lazy dog. No secrets here.";

        let actual = redact(fixture);

        assert_eq!(actual, fixture);
        assert!(matches!(actual, Cow::Borrowed(_)));
    }

    #[test]
    fn test_ordinary_code_without_a_sensitive_key_is_untouched() {
        let fixture = "fn tokenize(input: &str) -> Vec<String> { input.split(' ').collect() }";

        let actual = redact(fixture);

        assert_eq!(actual, fixture);
    }

    #[test]
    fn test_a_sentence_using_the_word_bearer_is_untouched() {
        let fixture = "The bond bearer receives interest payments annually from the issuer.";

        let actual = redact(fixture);

        assert_eq!(actual, fixture);
    }

    #[test]
    fn test_api_key_assignment_is_redacted() {
        let fixture = "api_key=sk-not-checked-here-abcdefghijklmnop next_arg=keep";

        let actual = redact(fixture);

        assert_eq!(actual, "api_key=[REDACTED] next_arg=keep");
    }

    #[test]
    fn test_json_field_named_api_key_is_redacted() {
        let fixture = r#"{"api_key": "abcdef123456", "region": "us-east-1"}"#;

        let actual = redact(fixture);

        assert_eq!(actual, r#"{"api_key": "[REDACTED]", "region": "us-east-1"}"#);
    }

    #[test]
    fn test_password_with_colon_form_is_redacted() {
        let fixture = "password: hunter2\nnext: line";

        let actual = redact(fixture);

        assert_eq!(actual, "password: [REDACTED]\nnext: line");
    }

    #[test]
    fn test_authorization_bearer_header_is_fully_redacted() {
        let fixture = "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";

        let actual = redact(fixture);

        assert_eq!(actual, "Authorization: [REDACTED]");
    }

    #[test]
    fn test_bearer_token_without_a_key_name_is_redacted() {
        let fixture = "curl -H 'Bearer abc123def456ghi789' https://api.example.com";

        let actual = redact(fixture);

        assert_eq!(actual, "curl -H '[REDACTED]' https://api.example.com");
    }

    #[test]
    fn test_secret_and_token_and_credential_keys_are_redacted() {
        let fixture = "secret=one token=two credential=three";

        let actual = redact(fixture);

        assert_eq!(actual, "secret=[REDACTED] token=[REDACTED] credential=[REDACTED]");
    }

    #[test]
    fn test_aws_access_key_is_redacted() {
        let fixture = "export AWS_ACCESS_KEY_ID=AKIAABCDEFGHIJKLMNOP";

        let actual = redact(fixture);

        assert_eq!(actual, "export AWS_ACCESS_KEY_ID=[REDACTED]");
    }

    #[test]
    fn test_aws_shaped_run_embedded_in_a_longer_identifier_is_not_matched() {
        let fixture = "XAKIAABCDEFGHIJKLMNOPvariablename";

        let actual = redact(fixture);

        assert_eq!(actual, fixture);
    }

    #[test]
    fn test_google_api_key_is_redacted() {
        let fixture = "key=AIzaSyD4nRj4KZs6xY8QwErTyUiOpAsDfGhJk12";

        let actual = redact(fixture);

        assert_eq!(actual, "key=[REDACTED]");
    }

    #[test]
    fn test_openai_style_key_is_redacted() {
        let fixture = "OPENAI_API_KEY=sk-abcdefghijklmnopqrstuvwxyz123456";

        let actual = redact(fixture);

        assert_eq!(actual, "OPENAI_API_KEY=[REDACTED]");
    }

    #[test]
    fn test_sk_shaped_run_embedded_in_a_hyphenated_word_is_not_matched() {
        let fixture = "ask-forgiveness-not-permission-please-and-thank-you";

        let actual = redact(fixture);

        assert_eq!(actual, fixture);
    }

    #[test]
    fn test_github_token_is_redacted() {
        let fixture = "ghp_1234567890abcdefghijklmnopqrstuvwxyzAB";

        let actual = redact(fixture);

        assert_eq!(actual, "[REDACTED]");
    }

    #[test]
    fn test_jwt_is_redacted() {
        let fixture = "session=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";

        let actual = redact(fixture);

        assert_eq!(actual, "session=[REDACTED]");
    }

    #[test]
    fn test_two_dot_json_like_text_without_a_real_jwt_shape_is_untouched() {
        let fixture = "eyJump.eyOver.the.fence";

        let actual = redact(fixture);

        assert_eq!(actual, fixture);
    }

    #[test]
    fn test_multiple_secrets_in_one_blob_are_all_redacted() {
        let fixture = "token=abc123\npassword=xyz789\nnormal_field=keep-me";

        let actual = redact(fixture);

        assert_eq!(actual, "token=[REDACTED]\npassword=[REDACTED]\nnormal_field=keep-me");
    }

    #[test]
    fn test_redact_json_replaces_sensitive_keys_at_any_depth() {
        let mut actual = json!({
            "user": "ada",
            "auth": {
                "api_key": "abc123",
                "nested": [{"password": "hunter2"}, {"note": "keep"}]
            }
        });

        redact_json(&mut actual);

        let expected = json!({
            "user": "ada",
            "auth": {
                "api_key": "[REDACTED]",
                "nested": [{"password": "[REDACTED]"}, {"note": "keep"}]
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_redact_json_leaves_a_tree_with_no_sensitive_keys_untouched() {
        let mut actual = json!({"path": "/tmp/x", "size": 42, "items": ["a", "b"]});
        let expected = actual.clone();

        redact_json(&mut actual);

        assert_eq!(actual, expected);
    }
}
