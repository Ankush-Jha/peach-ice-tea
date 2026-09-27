//! harness: T2.6 / R-TOOL-3 — correct misnamed tool arguments before
//! dispatch.
//!
//! Tool schemas reject unknown fields, so `filePath` or `contents` makes a
//! call fail, and recovering costs a whole model round trip. Peach already
//! normalises tool-name case, coerces types to the schema and accepts a few
//! aliases; what it cannot do is rename a key. This renames an unknown key to
//! a schema property only when the match is unambiguous, and never over a key
//! that is already present. Anything else is left for the normal error.
//!
//! Off unless [`ENV_VAR`] is `1` (principle 6: an A/B first).

use peach_domain::{ToolCallArguments, ToolCallFull, ToolCatalog};
use serde_json::{Map, Value};
use strum::IntoEnumIterator;

/// Environment variable that enables correction when set to `1`.
pub const ENV_VAR: &str = "PEACH_HARNESS_TOOL_CORRECTION";

/// Whether correction is enabled for this process.
pub fn enabled() -> bool {
    std::env::var(ENV_VAR).is_ok_and(|value| value == "1")
}

/// Names models commonly use for a schema property, by property.
const ALIASES: &[(&str, &[&str])] = &[
    (
        "file_path",
        &["filepath", "file", "filename", "path_to_file"],
    ),
    ("command", &["cmd", "shell_command", "script"]),
    (
        "old_string",
        &["old", "old_str", "old_text", "search", "find"],
    ),
    (
        "new_string",
        &["new", "new_str", "new_text", "replace", "replacement"],
    ),
    ("content", &["contents", "text", "data", "body"]),
    ("pattern", &["regex", "query", "search_pattern"]),
];

/// Renames misnamed keys of a built-in tool call. Returns the corrected call
/// and the renames made, as `(from, to)`.
pub fn correct(call: ToolCallFull) -> (ToolCallFull, Vec<(String, String)>) {
    let name = call.name.as_str().trim().to_ascii_lowercase();
    let Some(definition) = ToolCatalog::iter()
        .map(|tool| tool.definition())
        .find(|d| d.name.as_str() == name)
    else {
        return (call, vec![]);
    };
    let schema = serde_json::to_value(&definition.input_schema).unwrap_or_default();
    let properties: Vec<String> = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default();
    let Ok(Value::Object(arguments)) = call.arguments.parse() else {
        return (call, vec![]);
    };
    let (corrected, renames) = rename_keys(arguments, &properties);
    if renames.is_empty() {
        return (call, renames);
    }
    let arguments = ToolCallArguments::from(Value::Object(corrected));
    (call.arguments(arguments), renames)
}

/// The pure core of [`correct`]: renames keys not in `properties` to the one
/// property they unambiguously mean.
fn rename_keys(
    arguments: Map<String, Value>,
    properties: &[String],
) -> (Map<String, Value>, Vec<(String, String)>) {
    let mut out = Map::new();
    let mut renames = vec![];
    // Known keys first, so a rename can never overwrite one.
    for (key, value) in &arguments {
        if properties.contains(key) {
            out.insert(key.clone(), value.clone());
        }
    }
    for (key, value) in arguments {
        if properties.contains(&key) {
            continue;
        }
        match target(&key, properties).filter(|target| !out.contains_key(target)) {
            Some(target) => {
                renames.push((key, target.clone()));
                out.insert(target, value);
            }
            None => {
                out.insert(key, value);
            }
        }
    }
    (out, renames)
}

fn target(key: &str, properties: &[String]) -> Option<String> {
    let snake = to_snake(key);
    if properties.contains(&snake) {
        return Some(snake);
    }
    let alias = ALIASES
        .iter()
        .find(|(property, aliases)| {
            properties.iter().any(|p| p == property) && aliases.contains(&snake.as_str())
        })
        .map(|(property, _)| property.to_string());
    if alias.is_some() {
        return alias;
    }
    let near: Vec<&String> = properties
        .iter()
        .filter(|property| distance(&snake, property) <= 2)
        .collect();
    match near.as_slice() {
        [only] => Some((*only).clone()),
        _ => None,
    }
}

fn to_snake(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for (index, c) in key.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c == '-' {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out
}

/// Levenshtein distance.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut current = Vec::with_capacity(b.len() + 1);
        current.push(i + 1);
        // `previous` always has `b.len() + 1` entries, so each window is the
        // (diagonal, above) pair for one character of `b`.
        for (pair, cb) in previous.windows(2).zip(&b) {
            let [diagonal, above] = [
                pair.first().copied().unwrap_or(0),
                pair.last().copied().unwrap_or(0),
            ];
            let left = current.last().copied().unwrap_or(0);
            let cost = usize::from(ca != *cb);
            current.push((diagonal + cost).min(above + 1).min(left + 1));
        }
        previous = current;
    }
    previous.last().copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    fn properties(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn renamed(arguments: Value, names: &[&str]) -> (Value, Vec<(String, String)>) {
        let Value::Object(map) = arguments else {
            unreachable!()
        };
        let (out, renames) = rename_keys(map, &properties(names));
        (Value::Object(out), renames)
    }

    #[test]
    fn test_unambiguous_misnamed_keys_are_renamed() {
        let write = ["file_path", "content", "overwrite"];
        let patch = ["file_path", "old_string", "new_string", "replace_all"];

        let actual = vec![
            renamed(json!({"filePath": "/a", "contents": "x"}), &write).1,
            renamed(json!({"file_path": "/a", "old": "1", "new": "2"}), &patch).1,
            renamed(json!({"file_path": "/a", "contnet": "x"}), &write).1,
        ];

        let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(
            actual,
            vec![
                // serde_json's map keeps keys sorted, so renames come in key order.
                vec![pair("contents", "content"), pair("filePath", "file_path")],
                vec![pair("new", "new_string"), pair("old", "old_string")],
                vec![pair("contnet", "content")],
            ]
        );
    }

    #[test]
    fn test_ambiguous_unknown_or_colliding_keys_are_left_alone() {
        // `x_string` is within distance 2 of both old_string and new_string.
        let patch = ["file_path", "old_string", "new_string"];
        let write = ["file_path", "content"];

        let actual = vec![
            renamed(json!({"file_path": "/a", "x_string": "?"}), &patch).1,
            renamed(json!({"file_path": "/a", "banana": 1}), &write).1,
            // `filepath` would map to file_path, but file_path is already there.
            renamed(json!({"file_path": "/a", "filepath": "/b"}), &write).1,
        ];

        assert_eq!(actual, vec![vec![], vec![], vec![]]);
        let (kept, _) = renamed(json!({"file_path": "/a", "filepath": "/b"}), &write);
        assert_eq!(kept, json!({"file_path": "/a", "filepath": "/b"}));
    }

    #[test]
    fn test_real_catalog_calls_are_corrected_and_others_untouched() {
        let misnamed = ToolCallFull::new("write").arguments(ToolCallArguments::from(
            json!({"filePath": "/tmp/a", "contents": "x"}),
        ));
        let correct_call = ToolCallFull::new("read")
            .arguments(ToolCallArguments::from(json!({"file_path": "/tmp/a"})));
        let mcp = ToolCallFull::new("mcp_github_search")
            .arguments(ToolCallArguments::from(json!({"q": 1})));

        let (fixed, renames) = correct(misnamed);
        let untouched: Vec<usize> = [correct_call, mcp]
            .into_iter()
            .map(|call| correct(call).1.len())
            .collect();

        assert_eq!(renames.len(), 2);
        assert_eq!(
            fixed.arguments.parse().unwrap(),
            json!({"file_path": "/tmp/a", "content": "x"})
        );
        assert_eq!(untouched, vec![0, 0]);
    }
}
