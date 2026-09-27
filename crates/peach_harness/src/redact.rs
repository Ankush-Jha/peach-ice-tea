//! Secret redaction before anything leaves the process (`R-SAFE-3`).
//!
//! Every tool-call preview that might reach a relevance scorer, an external
//! hook, or a log line passes through here first (`DECISIONS.md` D-027,
//! S2's "secrets redacted from input previews sent to the scorer"). The scan
//! is plain string matching over bytes — no regex crate is available to this
//! crate — and is written so every pass is linear in the input length, even
//! against adversarial input, so it stays cheap to run ahead of every
//! scorer call with no timeout of its own.
//!
//! Several complementary strategies are used:
//! - **Key-shaped matches**: an identifier that looks like a secret-bearing
//!   key (`api_key`, `token`, `password`, ...) immediately followed by a
//!   separator (`=`, `:`, `": "`) redacts the value that follows. This is
//!   deliberately biased toward over-redaction for the sake of recall: a
//!   generic word like `token` or `secret` used as an ordinary variable or
//!   config-key name can get its value redacted even though it isn't a
//!   secret (`token = generate_uuid()`, `credential_store = cache`) —
//!   leaking a real secret is worse than an unnecessary `[REDACTED]`. Two
//!   narrower exceptions exist specifically to avoid *corrupting* source
//!   code rather than just over-redacting it: a lone identifier is never
//!   treated as a key when it is a Rust/TS-style type annotation (`name:
//!   Type`, so `pub token: String` is left alone) or reached via `::` (a
//!   path/type reference, not an assignment, so `Type::method(...)` after
//!   `=` is never swallowed as a "value").
//! - **Value-shaped matches**: well-known credential formats (AWS, Google,
//!   OpenAI-style, GitHub, JWT, bearer tokens, PEM/OpenSSH private key
//!   blocks, Slack webhook URLs) are recognisable from their own shape and
//!   are redacted wherever they appear, even with no key name nearby. Two of
//!   these — the OpenAI-style and JWT shapes — use a greedy scan whose
//!   alphabet includes their own prefix's characters, which would otherwise
//!   let a repeated prefix (`"sk-".repeat(n)`, `"-eyJ".repeat(n)`) cause
//!   quadratic rescans; both track a "dead zone" so a rejected run is never
//!   rescanned from a later starting point inside it.
//! - **Positional matches**: connection-string credentials
//!   (`scheme://user:pass@host`) have no key name and no fixed shape beyond
//!   their position between `://` and `@`.

use std::borrow::Cow;

const REDACTED: &str = "[REDACTED]";

/// Key substrings (already lower-cased, `-` normalised to `_`) that mark a
/// key/value pair as secret-bearing. Matching is substring containment, so
/// e.g. `x-api-key` and `apiKeyForUser` both match `api_key`, and
/// `JSESSIONID` matches `session`.
const SENSITIVE_KEY_SUBSTRINGS: &[&str] = &[
    "api_key",
    "apikey",
    "token",
    "password",
    "passwd",
    "secret",
    "authorization",
    "auth",
    "bearer",
    "credential",
    "private_key",
    "session",
    "cookie",
    "dsn",
    "connection_string",
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
        output.push_str(text(input, cursor, start));
        output.push_str(REDACTED);
        cursor = end;
    }
    output.push_str(input.get(cursor..).unwrap_or_default());
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

/// Redacts a JSON document for writing to disk without changing its shape:
/// every string leaf goes through [`redact`], and a string value under a
/// secret-looking key is replaced outright. Numbers, booleans and structure
/// are never touched.
///
/// Use this, not [`redact_json`], for documents that carry counters: a key
/// test alone would replace `input_tokens: 1200` (`token` is a sensitive
/// substring) and running [`redact`] over serialized JSON text could turn
/// `"token_count": 5` into invalid JSON.
pub fn redact_json_strings(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, entry) in map.iter_mut() {
                match entry {
                    serde_json::Value::String(_) if is_sensitive_key(key) => {
                        *entry = serde_json::Value::String(REDACTED.to_string());
                    }
                    _ => redact_json_strings(entry),
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(redact_json_strings),
        serde_json::Value::String(text) => {
            if let Cow::Owned(redacted) = redact(text) {
                *text = redacted;
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace('-', "_");
    SENSITIVE_KEY_SUBSTRINGS.iter().any(|needle| normalized.contains(needle))
}

// Bounds-checked accessors (CI's `indexing_slicing` / `string_slice` lints).
// For any in-range index they return exactly what indexing did; past the end
// they return 0 or an empty slice instead of panicking, so no input can
// crash redaction. Span boundaries always fall on ASCII bytes, which are
// always UTF-8 char boundaries, so `text` never sees a split character.

/// `bytes[index]`, or 0 (never an identifier, gap or token byte) past the end.
fn at(bytes: &[u8], index: usize) -> u8 {
    bytes.get(index).copied().unwrap_or(0)
}

/// `bytes[from..to]`, or empty when out of range.
fn span(bytes: &[u8], from: usize, to: usize) -> &[u8] {
    bytes.get(from..to).unwrap_or_default()
}

/// `bytes[from..]`, or empty when out of range.
fn tail(bytes: &[u8], from: usize) -> &[u8] {
    bytes.get(from..).unwrap_or_default()
}

/// `input[from..to]`, or empty when out of range.
fn text(input: &str, from: usize, to: usize) -> &str {
    input.get(from..to).unwrap_or_default()
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

fn is_gap_byte(byte: u8) -> bool {
    byte == b'"' || byte == b'\'' || byte.is_ascii_whitespace()
}

fn preceded_by_word_char(bytes: &[u8], index: usize) -> bool {
    index > 0 && at(bytes, index - 1).is_ascii_alphanumeric()
}

/// Finds every byte span that should be replaced with `[REDACTED]`, in the
/// order found. Spans may overlap; `merge_spans` reconciles that.
fn find_spans(input: &str) -> Vec<(usize, usize)> {
    let mut spans = find_key_value_spans(input);
    find_value_shape_spans(input, &mut spans);
    find_pem_private_key_spans(input, &mut spans);
    find_connection_string_spans(input, &mut spans);
    spans
}

/// Pass 1: `key = value`, `key: value` and `"key": "value"` shapes, where
/// `key` contains one of `SENSITIVE_KEY_SUBSTRINGS`.
///
/// Two shapes never count as a key, no matter what they contain: a lone
/// identifier immediately preceded (modulo whitespace) by a single `:` is a
/// type annotation (`name: Type`), not a key, so the `Type` in `let name:
/// Type = value` is never checked against `= value`; and `::` is always a
/// path/type separator, never a key/value separator, so `Type::method(...)`
/// is never swallowed as the "value" of whatever preceded it.
fn find_key_value_spans(input: &str) -> Vec<(usize, usize)> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut spans = Vec::new();

    let mut word_start: Option<usize> = None;
    let mut last_word: Option<(usize, usize)> = None;
    let mut last_word_is_type_annotation = false;
    let mut pending_after_single_colon = false;
    let mut i = 0usize;

    while i < len {
        let byte = at(bytes, i);

        if is_ident_byte(byte) {
            if word_start.is_none() {
                word_start = Some(i);
            }
            i += 1;
            continue;
        }

        if let Some(start) = word_start.take() {
            last_word = Some((start, i));
            last_word_is_type_annotation = pending_after_single_colon;
            pending_after_single_colon = false;
        }

        if byte == b':' {
            if bytes.get(i + 1) == Some(&b':') {
                // `::` is a path separator (`std::env`, `Type::method`),
                // never a key/value separator.
                last_word = None;
                last_word_is_type_annotation = false;
                pending_after_single_colon = false;
                i += 2;
                continue;
            }

            if let Some(next) =
                try_redact_after_separator(input, bytes, last_word, last_word_is_type_annotation, i + 1, &mut spans)
            {
                last_word = None;
                last_word_is_type_annotation = false;
                pending_after_single_colon = false;
                i = next;
                continue;
            }

            last_word = None;
            last_word_is_type_annotation = false;
            pending_after_single_colon = true;
            i += 1;
            continue;
        }

        if byte == b'=' {
            if let Some(next) =
                try_redact_after_separator(input, bytes, last_word, last_word_is_type_annotation, i + 1, &mut spans)
            {
                last_word = None;
                last_word_is_type_annotation = false;
                pending_after_single_colon = false;
                i = next;
                continue;
            }

            last_word = None;
            last_word_is_type_annotation = false;
            pending_after_single_colon = false;
            i += 1;
            continue;
        }

        if is_gap_byte(byte) {
            // Whitespace and quote characters between a key and its
            // separator don't invalidate the key (`"api_key" : "x"`,
            // `token = x`) or the pending-colon state, so keep both alive
            // across them.
            i += 1;
            continue;
        }

        last_word = None;
        last_word_is_type_annotation = false;
        pending_after_single_colon = false;
        i += 1;
    }

    spans
}

/// Attempts to redact the value following a `=`/`:` separator at
/// `scan_from`, when `candidate_key` is sensitive and isn't itself a type
/// annotation. Returns the index scanning should resume from on success —
/// whether or not a span was actually pushed, since a type-like value or an
/// empty value is not an error — or `None` if this was never a key/value
/// pair (no candidate, not sensitive, or a type annotation), in which case
/// the caller advances past the separator itself.
fn try_redact_after_separator(
    input: &str,
    bytes: &[u8],
    candidate_key: Option<(usize, usize)>,
    candidate_is_type_annotation: bool,
    scan_from: usize,
    spans: &mut Vec<(usize, usize)>,
) -> Option<usize> {
    let (word_from, word_to) = candidate_key?;
    if candidate_is_type_annotation {
        return None;
    }
    let key = text(input, word_from, word_to);
    if !is_sensitive_key(key) {
        return None;
    }

    let (value_start, value_end, next, was_quoted, crossed_newline) = parse_value(bytes, scan_from)?;
    // An unquoted token that is nothing but a capitalised identifier reads
    // as a bare type or path reference (`String`, `SecretKey`, `Vec`)
    // rather than a secret value, and an unquoted token immediately
    // followed by `::` is unambiguously a path segment (`std::env`,
    // whatever its case) — real secret values are either quoted or contain
    // digits/symbols that a plain identifier doesn't. Leave both alone
    // rather than guess wrong on ordinary source (`RESEARCH.md` S5's
    // git-diff lesson about over-aggressive compression).
    let followed_by_path_separator = bytes.get(value_end) == Some(&b':') && bytes.get(value_end + 1) == Some(&b':');
    // The type-reference heuristic reads code shape (`let k: SecretKey = ...`).
    // A value that began on a later line is config shape (`password=\nValue`),
    // where a bare capitalised identifier is an ordinary secret, not a type.
    let looks_like_a_type_reference = !was_quoted
        && !crossed_newline
        && (is_bare_type_like(span(bytes, value_start, value_end)) || followed_by_path_separator);
    if !looks_like_a_type_reference && value_start < value_end {
        spans.push((value_start, value_end));
    }
    Some(next)
}

fn is_bare_type_like(token: &[u8]) -> bool {
    matches!(token.first(), Some(b) if b.is_ascii_uppercase()) && token.iter().all(|&b| is_ident_byte(b))
}

fn skip_inline_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && (at(bytes, i) == b' ' || at(bytes, i) == b'\t') {
        i += 1;
    }
    i
}

/// Parses the value following a `=`/`:` separator, starting at `i`.
///
/// Returns `(value_start, value_end, next_index, was_quoted)`: the
/// redactable span (excluding surrounding quotes, if any), the index
/// scanning should resume from, and whether the value was quoted. Leading
/// spaces/tabs are skipped, and — since a value can begin on the next line
/// (`password=\nSomeSecretValue`) — at most one line break plus its
/// following indentation is skipped too, before value parsing starts. A
/// quoted value ends at its matching (unescaped) closing quote; an unquoted
/// value ends at whitespace or a small set of separators (`&`, `;`, `,`,
/// `}`, `)`, `]`, `:`, `(`) common to query strings, shell assignments and
/// structured text — deliberately not consuming across a space, so an
/// inline shell assignment like `API_KEY=x command --flag` doesn't swallow
/// the rest of the line, and stopping at `:`/`(` so a code expression like
/// `Type::method(args)` after an `=` doesn't get swept up whole (the caller
/// separately recognises a value ending right before `::` as a path
/// reference rather than redacting the fragment before it).
fn parse_value(bytes: &[u8], mut i: usize) -> Option<(usize, usize, usize, bool, bool)> {
    let len = bytes.len();
    i = skip_inline_whitespace(bytes, i);

    let mut crossed_newline = false;
    if i < len && (at(bytes, i) == b'\n' || at(bytes, i) == b'\r') {
        crossed_newline = true;
        if at(bytes, i) == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            i += 2;
        } else {
            i += 1;
        }
        i = skip_inline_whitespace(bytes, i);
    }

    if i >= len {
        return Some((i, i, i, false, crossed_newline));
    }

    if at(bytes, i) == b'"' || at(bytes, i) == b'\'' {
        let quote = at(bytes, i);
        let start = i + 1;
        let mut j = start;
        while j < len {
            if at(bytes, j) == b'\\' && j + 1 < len {
                j += 2;
                continue;
            }
            if at(bytes, j) == quote {
                break;
            }
            j += 1;
        }
        let end = j.min(len);
        let next = if j < len { j + 1 } else { j };
        return Some((start, end, next, true, crossed_newline));
    }

    let start = i;
    while i < len {
        let byte = at(bytes, i);
        if byte.is_ascii_whitespace() || matches!(byte, b'&' | b';' | b',' | b'}' | b')' | b']' | b':' | b'(') {
            break;
        }
        i += 1;
    }
    Some((start, i, i, false, crossed_newline))
}

/// Pass 2: recognisable secret shapes, independent of any surrounding key.
///
/// `eyJ` (JWT) greedily consumes a base64url run and only then checks
/// whether a `.` follows — a check entirely independent of how long the run
/// was. That decoupling is what makes it possible for a long run to cost a
/// lot to scan *and* still fail (no `.` anywhere), and for the literal
/// prefix to recur inside that same run with nothing to break it up (`-`
/// is valid base64url, so `"-eyJ".repeat(n)` is one uninterrupted run).
/// Retrying the JWT matcher at every position inside such a run would make
/// one rejected run get rescanned from every later position inside it,
/// which is quadratic (measured: this exact input, before the fix below).
/// Once a run has been scanned and rejected, this function remembers where
/// it ends and skips straight past it.
///
/// Every other matcher here is retried at every position unconditionally,
/// because none of them share that decoupling: AWS and Google keys are
/// fixed-length checks with no greedy scan at all; OpenAI-style and GitHub
/// tokens both succeed purely on how much of their own run they managed to
/// consume (`count >= threshold`), so a run long enough to be expensive to
/// scan is, by that same length, long enough to succeed — there is no input
/// that is both expensive and a failure for either of them; and the bearer
/// matcher requires actual whitespace immediately after the word, which
/// breaks any such run on its own.
fn find_value_shape_spans(input: &str, spans: &mut Vec<(usize, usize)>) {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut jwt_dead_zone_end = 0usize;
    let mut openai_dead_zone_end = 0usize;

    while i < len {
        let matched = match_aws_key(bytes, i)
            .or_else(|| if i >= openai_dead_zone_end { match_openai_key(bytes, i) } else { None })
            .or_else(|| match_google_key(bytes, i))
            .or_else(|| match_github_token(bytes, i))
            .or_else(|| match_bearer_token(bytes, i))
            .map(|end| (i, end))
            .or_else(|| match_slack_webhook(bytes, i));
        if let Some((span_start, end)) = matched {
            spans.push((span_start, end));
            i = end;
            continue;
        }

        // An OpenAI-shaped run that failed (no digit) is expensive to scan and
        // can recur at every offset inside itself, so remember where it ends
        // rather than rescanning it from each one. Gated on the dead zone
        // itself, or `sk-` recurring inside the run just rejected (every 3
        // bytes here) would redo this same full-length scan from each
        // occurrence — exactly the quadratic blowup this exists to avoid.
        if i >= openai_dead_zone_end && tail(bytes, i).starts_with(b"sk-") {
            openai_dead_zone_end = ident_run_end(bytes, i);
        }

        if i >= jwt_dead_zone_end {
            if let Some(end) = match_jwt(bytes, i) {
                spans.push((i, end));
                i = end;
                continue;
            }
            if tail(bytes, i).starts_with(b"eyJ") {
                jwt_dead_zone_end = base64url_run_end(bytes, i);
            }
        }

        i += 1;
    }
}

fn ident_run_end(bytes: &[u8], start: usize) -> usize {
    let mut j = start;
    while j < bytes.len() && is_ident_byte(at(bytes, j)) {
        j += 1;
    }
    j
}

fn base64url_run_end(bytes: &[u8], start: usize) -> usize {
    let mut j = start;
    while j < bytes.len() && is_base64url_byte(at(bytes, j)) {
        j += 1;
    }
    j
}

/// AWS access key: `AKIA` followed by exactly 16 `[0-9A-Z]` characters.
fn match_aws_key(bytes: &[u8], i: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"AKIA";
    if preceded_by_word_char(bytes, i) || !tail(bytes, i).starts_with(PREFIX) {
        return None;
    }
    let start = i + PREFIX.len();
    let end = start.checked_add(16)?;
    if end > bytes.len() {
        return None;
    }
    if span(bytes, start, end).iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
        Some(end)
    } else {
        None
    }
}

/// Google API key: `AIza` followed by exactly 35 `[0-9A-Za-z_-]` characters.
fn match_google_key(bytes: &[u8], i: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"AIza";
    if preceded_by_word_char(bytes, i) || !tail(bytes, i).starts_with(PREFIX) {
        return None;
    }
    let start = i + PREFIX.len();
    let end = start.checked_add(35)?;
    if end > bytes.len() {
        return None;
    }
    if span(bytes, start, end).iter().all(|&b| is_ident_byte(b)) {
        Some(end)
    } else {
        None
    }
}

/// OpenAI-style key: `sk-` followed by 20 or more `[A-Za-z0-9_-]` characters.
fn match_openai_key(bytes: &[u8], i: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"sk-";
    if preceded_by_word_char(bytes, i) || !tail(bytes, i).starts_with(PREFIX) {
        return None;
    }
    let start = i + PREFIX.len();
    let end = ident_run_end(bytes, start);
    if end - start < 20 {
        return None;
    }
    // Real keys carry digits; requiring one keeps a run of repeated `sk-`
    // from reading as a key.
    if !span(bytes, start, end).iter().any(u8::is_ascii_digit) {
        return None;
    }
    Some(end)
}

/// GitHub token: `gh` + one of `p`/`o`/`u`/`s`/`r` + `_` + 36 or more
/// alphanumeric characters.
fn match_github_token(bytes: &[u8], i: usize) -> Option<usize> {
    if preceded_by_word_char(bytes, i) || !tail(bytes, i).starts_with(b"gh") {
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
    while end < bytes.len() && at(bytes, end).is_ascii_alphanumeric() {
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
    if preceded_by_word_char(bytes, i) || !tail(bytes, i).starts_with(PREFIX) {
        return None;
    }
    let len = bytes.len();
    let j = base64url_run_end(bytes, i);
    if j - i < MIN_JWT_SEGMENT_LEN || j >= len || at(bytes, j) != b'.' {
        return None;
    }
    let segment_two_start = j + 1;
    let j = base64url_run_end(bytes, segment_two_start);
    if j - segment_two_start < MIN_JWT_SEGMENT_LEN || j >= len || at(bytes, j) != b'.' {
        return None;
    }
    let segment_three_start = j + 1;
    let j = base64url_run_end(bytes, segment_three_start);
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
    if at(bytes, i) != b'B' && at(bytes, i) != b'b' {
        return None;
    }
    if preceded_by_word_char(bytes, i) || i + WORD.len() > len {
        return None;
    }
    for (offset, expected) in WORD.iter().enumerate() {
        if !at(bytes, i + offset).eq_ignore_ascii_case(expected) {
            return None;
        }
    }

    let mut j = i + WORD.len();
    if j >= len || !at(bytes, j).is_ascii_whitespace() {
        return None;
    }
    while j < len && at(bytes, j).is_ascii_whitespace() {
        j += 1;
    }

    let token_start = j;
    while j < len && is_token_byte(at(bytes, j)) {
        j += 1;
    }
    if j - token_start < 16 {
        return None;
    }
    let token = span(bytes, token_start, j);
    let looks_like_a_token =
        token.iter().any(|b| b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.' | b'+' | b'/' | b'='));
    if !looks_like_a_token {
        return None;
    }

    Some(j)
}

/// Slack incoming-webhook URL: `hooks.slack.com/services/` followed by its
/// path segments (workspace/channel/token identifiers), which are the
/// credential — anyone with the URL can post as the configured integration.
fn match_slack_webhook(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    const MARKER: &[u8] = b"hooks.slack.com/services/";
    if preceded_by_word_char(bytes, i) || !tail(bytes, i).starts_with(MARKER) {
        return None;
    }
    let path_start = i + MARKER.len();
    let mut j = path_start;
    while j < bytes.len() && (at(bytes, j).is_ascii_alphanumeric() || at(bytes, j) == b'/') {
        j += 1;
    }
    if j - path_start < 10 { None } else { Some((path_start, j)) }
}

/// Bounded forward search for `needle` starting at `start`, scanning at
/// most `max_scan` bytes of haystack (plus the needle's own length) so a
/// caller can bound the cost of a search that might otherwise never find
/// its target in adversarial input.
fn find_subsequence_bounded(bytes: &[u8], start: usize, needle: &[u8], max_scan: usize) -> Option<usize> {
    if needle.is_empty() || start >= bytes.len() {
        return None;
    }
    let search_end = bytes.len().min(start.saturating_add(max_scan).saturating_add(needle.len()));
    span(bytes, start, search_end).windows(needle.len()).position(|window| window == needle).map(|pos| start + pos)
}

/// Unbounded forward search for `needle` starting at `start`. Safe to call
/// once per call site in a loop that only ever moves `start` forward,
/// because the combined cost across all such calls is then still linear in
/// the input (each byte is examined by at most one call).
fn find_subsequence(bytes: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    find_subsequence_bounded(bytes, start, needle, bytes.len())
}

fn contains_ascii_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|window| window.eq_ignore_ascii_case(needle))
}

/// Maximum distance searched, from the end of a `-----BEGIN ...-----` line,
/// for the corresponding `-----END ...-----`. Bounds the cost of a single
/// unmatched `BEGIN` marker; real PEM/OpenSSH private keys are a few
/// kilobytes at most.
const PEM_BODY_SEARCH_CAP: usize = 8_000;
/// Maximum distance searched for the `-----` that closes a `BEGIN`/`END`
/// label line. Labels are a handful of words.
const PEM_LABEL_SEARCH_CAP: usize = 256;

/// Pass 3: PEM/OpenSSH private key blocks (`-----BEGIN ... PRIVATE
/// KEY-----` through the matching `-----END ... PRIVATE KEY-----`).
///
/// Only blocks whose label contains "PRIVATE KEY" are treated as secrets —
/// certificates and public keys are not. Both the label search and the
/// matching-`END` search are bounded (see the constants above), and the
/// outer scan for the next `BEGIN` marker only ever moves forward, so this
/// whole pass stays linear even against many unmatched `BEGIN` markers.
fn find_pem_private_key_spans(input: &str, spans: &mut Vec<(usize, usize)>) {
    const BEGIN: &[u8] = b"-----BEGIN ";
    const END: &[u8] = b"-----END ";
    const DASHES: &[u8] = b"-----";

    let bytes = input.as_bytes();
    let mut i = 0usize;

    while let Some(begin_pos) = find_subsequence(bytes, i, BEGIN) {
        let label_start = begin_pos + BEGIN.len();
        let Some(label_dashes) = find_subsequence_bounded(bytes, label_start, DASHES, PEM_LABEL_SEARCH_CAP) else {
            i = label_start;
            continue;
        };
        let begin_line_end = label_dashes + DASHES.len();
        let label = span(bytes, label_start, label_dashes);

        if !contains_ascii_ignore_case(label, b"PRIVATE KEY") {
            i = begin_line_end;
            continue;
        }

        let matched_end = find_subsequence_bounded(bytes, begin_line_end, END, PEM_BODY_SEARCH_CAP)
            .and_then(|end_pos| {
                let end_label_start = end_pos + END.len();
                find_subsequence_bounded(bytes, end_label_start, DASHES, PEM_LABEL_SEARCH_CAP)
                    .map(|end_dashes| end_dashes + DASHES.len())
            });

        match matched_end {
            Some(block_end) => {
                spans.push((begin_pos, block_end));
                i = block_end;
            }
            None => {
                i = begin_line_end;
            }
        }
    }
}

/// Maximum distance searched, from `scheme://`, for the `@` that ends a
/// `user:pass@` authority section. Authority sections are short.
const CONNECTION_STRING_AUTHORITY_SEARCH_CAP: usize = 512;

/// Pass 4: `scheme://user:pass@host` connection strings
/// (`postgresql://admin:S3cr3t@host:5432/db`). These have no recognisable
/// key name and no fixed credential shape, only a position — between `://`
/// and `@` — so this redacts just the password portion of that position,
/// leaving the scheme, username and host visible.
fn find_connection_string_spans(input: &str, spans: &mut Vec<(usize, usize)>) {
    const MARKER: &[u8] = b"://";
    let bytes = input.as_bytes();
    let mut i = 0usize;

    while let Some(marker_pos) = find_subsequence(bytes, i, MARKER) {
        let userinfo_start = marker_pos + MARKER.len();
        let cap_end = bytes.len().min(userinfo_start + CONNECTION_STRING_AUTHORITY_SEARCH_CAP);

        let mut at_pos = None;
        let mut k = userinfo_start;
        while k < cap_end {
            match at(bytes, k) {
                b'@' => {
                    at_pos = Some(k);
                    break;
                }
                b if b.is_ascii_whitespace() || b == b'/' || b == b'"' || b == b'\'' => break,
                _ => k += 1,
            }
        }

        match at_pos {
            Some(at) => {
                let userinfo = span(bytes, userinfo_start, at);
                if let Some(colon_rel) = userinfo.iter().position(|&b| b == b':') {
                    let pass_start = userinfo_start + colon_rel + 1;
                    if pass_start < at {
                        spans.push((pass_start, at));
                    }
                }
                i = at + 1;
            }
            None => {
                i = cap_end.max(userinfo_start);
            }
        }
    }
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
    use std::time::Instant;

    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_json_strings_are_redacted_and_counters_are_not() {
        let key = format!("AIza{}", "k".repeat(35));
        let mut fixture = serde_json::json!({
            "input_tokens": 1200,
            "api_key": "plain-secret-value",
            "messages": [{"text": format!("use {key} here")}],
            "nested": {"session_count": 3},
        });

        redact_json_strings(&mut fixture);

        let expected = serde_json::json!({
            "input_tokens": 1200,
            "api_key": "[REDACTED]",
            "messages": [{"text": "use [REDACTED] here"}],
            "nested": {"session_count": 3},
        });
        assert_eq!(fixture, expected);
    }

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
    fn test_rust_type_annotation_assigned_from_its_own_constructor_is_untouched() {
        let fixture = "let key: SecretKey = SecretKey::generate(&mut rng);";

        let actual = redact(fixture);

        assert_eq!(actual, fixture);
    }

    #[test]
    fn test_struct_field_named_token_with_a_type_annotation_is_untouched() {
        let fixture = "pub token: String,";

        let actual = redact(fixture);

        assert_eq!(actual, fixture);
    }

    #[test]
    fn test_path_qualified_call_after_an_assignment_is_untouched() {
        let fixture = "let token = std::env::var(\"UNRELATED\").unwrap();";

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
    fn test_password_value_starting_on_the_next_line_is_redacted() {
        let fixture = "password=\nSomeSecretValue\nnext_field=keep";

        let actual = redact(fixture);

        assert_eq!(actual, "password=\n[REDACTED]\nnext_field=keep");
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
    fn test_x_auth_header_is_redacted() {
        let fixture = "x-auth: s3cr3t-value-here";

        let actual = redact(fixture);

        assert_eq!(actual, "x-auth: [REDACTED]");
    }

    #[test]
    fn test_set_cookie_header_is_redacted() {
        let fixture = "Set-Cookie: session=abc123def456; Path=/; HttpOnly";

        let actual = redact(fixture);

        assert_eq!(actual, "Set-Cookie: [REDACTED]; Path=/; HttpOnly");
    }

    #[test]
    fn test_jsessionid_is_redacted() {
        let fixture = "JSESSIONID=1A2B3C4D5E6F7G8H9I0J";

        let actual = redact(fixture);

        assert_eq!(actual, "JSESSIONID=[REDACTED]");
    }

    #[test]
    fn test_postgres_connection_string_password_is_redacted() {
        let fixture = "postgresql://admin:S3cr3tPassw0rd@host:5432/db";

        let actual = redact(fixture);

        assert_eq!(actual, "postgresql://admin:[REDACTED]@host:5432/db");
    }

    #[test]
    fn test_connection_string_without_credentials_is_untouched() {
        let fixture = "https://example.com/path?query=1";

        let actual = redact(fixture);

        assert_eq!(actual, fixture);
    }

    #[test]
    fn test_openssh_private_key_block_is_redacted() {
        let fixture = "before\n-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXkAAAAA\nAAAAAAAAAAAAAAAAAAAA\n-----END OPENSSH PRIVATE KEY-----\nafter";

        let actual = redact(fixture);

        assert_eq!(actual, "before\n[REDACTED]\nafter");
    }

    #[test]
    fn test_pem_public_certificate_block_is_untouched() {
        let fixture = "before\n-----BEGIN CERTIFICATE-----\nMIIBIjANBgkqhkiG9w0B\n-----END CERTIFICATE-----\nafter";

        let actual = redact(fixture);

        assert_eq!(actual, fixture);
    }

    #[test]
    fn test_slack_webhook_url_is_redacted() {
        let fixture = "https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX";

        let actual = redact(fixture);

        assert_eq!(actual, "https://hooks.slack.com/services/[REDACTED]");
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
        let fixture = "session_value=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";

        let actual = redact(fixture);

        assert_eq!(actual, "session_value=[REDACTED]");
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
            "settings": {
                "api_key": "abc123",
                "nested": [{"password": "hunter2"}, {"note": "keep"}]
            }
        });

        redact_json(&mut actual);

        let expected = json!({
            "user": "ada",
            "settings": {
                "api_key": "[REDACTED]",
                "nested": [{"password": "[REDACTED]"}, {"note": "keep"}]
            }
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_redact_json_replaces_a_key_that_is_itself_sensitive_wholesale() {
        let mut actual = json!({"user": "ada", "auth": {"scheme": "bearer", "value": "abc123"}});

        redact_json(&mut actual);

        let expected = json!({"user": "ada", "auth": "[REDACTED]"});
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_redact_json_leaves_a_tree_with_no_sensitive_keys_untouched() {
        let mut actual = json!({"path": "/tmp/x", "size": 42, "items": ["a", "b"]});
        let expected = actual.clone();

        redact_json(&mut actual);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_pathological_repeated_eyj_prefix_completes_quickly() {
        let fixture = "-eyJ".repeat(50_000); // ~200 KB, no `.` anywhere

        let started = Instant::now();
        let actual = redact(&fixture);
        let elapsed = started.elapsed();

        assert_eq!(actual, fixture);
        assert!(elapsed.as_secs() < 2, "redact took {elapsed:?} on pathological JWT-shaped input");
    }

    #[test]
    fn test_pathological_repeated_sk_prefix_completes_quickly() {
        let fixture = "sk-".repeat(50_000); // ~150 KB, always < 20 chars between repeats... but forms one long run

        let started = Instant::now();
        let actual = redact(&fixture);
        let elapsed = started.elapsed();

        assert_eq!(actual, fixture);
        assert!(elapsed.as_secs() < 2, "redact took {elapsed:?} on pathological OpenAI-key-shaped input");
    }
}
