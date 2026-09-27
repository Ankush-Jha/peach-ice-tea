//! Test-only compatibility checks between Peach's tool schemas (plus a
//! representative MCP-style fixture) and Gemini's function-declaration
//! Schema (TH.3, R-HACK-6, T0.8).
//!
//! `FunctionDeclaration::from(ToolDefinition)` (in `super::request`) runs
//! every tool's raw JSON Schema through `crate::utils::sanitize_gemini_schema`
//! before it is sent on the wire. Gemini's Schema is documented as an
//! OpenAPI-3.0-style subset: unsupported keys or `format` values risk a 400
//! on every request, which in a one-shot evaluation is total failure. This
//! module walks the *converted* declaration (not the raw schema) and asserts
//! nothing unsupported survived.

#![cfg(test)]

use peach_domain::{ToolCatalog, ToolDefinition};
use pretty_assertions::assert_eq;
use schemars::Schema;
use strum::IntoEnumIterator;

use super::request::FunctionDeclaration;

/// Keys Gemini's function-declaration Schema documents. Anything else
/// surviving `FunctionDeclaration::from` is a defect — Gemini rejects
/// unrecognised keys.
const ALLOWED_GEMINI_SCHEMA_KEYS: &[&str] = &[
    "type",
    "format",
    "description",
    "nullable",
    "enum",
    "properties",
    "required",
    "items",
    "minItems",
    "maxItems",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "pattern",
    "anyOf",
    "default",
    "propertyOrdering",
    "minProperties",
    "maxProperties",
    "example",
];

/// Mirrors the type/format table Gemini's function-declaration Schema
/// documents: int32/int64 for INTEGER, float/double for NUMBER, and
/// enum/date-time for STRING.
fn is_supported_format(type_str: Option<&str>, format: &str) -> bool {
    matches!(
        (type_str, format),
        (Some("integer"), "int32" | "int64")
            | (Some("number"), "float" | "double")
            | (Some("string"), "enum" | "date-time")
    )
}

/// Recursively walks a converted Gemini schema value, asserting:
/// - every object's keys are in `ALLOWED_GEMINI_SCHEMA_KEYS`;
/// - every `format` is one Gemini documents for its `type`;
/// - every `required` entry has a matching entry in `properties`, and
///   `required` never appears without `properties`.
fn assert_gemini_compatible(value: &serde_json::Value, path: &str) {
    // Only object schema nodes carry keys worth checking. Non-object values
    // reached from here are payloads, not schema nodes — `enum`'s values,
    // an `items: false` etc. — and are recursed into only from the
    // targeted places below, never generically.
    let serde_json::Value::Object(map) = value else {
        return;
    };

    for key in map.keys() {
        assert!(
            ALLOWED_GEMINI_SCHEMA_KEYS.contains(&key.as_str()),
            "{path}: key {key:?} is not one Gemini's function-declaration Schema documents"
        );
    }

    if let Some(format) = map.get("format").and_then(|v| v.as_str()) {
        let type_str = map.get("type").and_then(|v| v.as_str());
        assert!(
            is_supported_format(type_str, format),
            "{path}: format {format:?} is not supported for type {type_str:?}"
        );
    }

    match (
        map.get("required").and_then(|v| v.as_array()),
        map.get("properties").and_then(|v| v.as_object()),
    ) {
        (Some(required), Some(properties)) => {
            for entry in required {
                let name = entry.as_str().unwrap_or_default();
                assert!(
                    properties.contains_key(name),
                    "{path}: required entry {name:?} has no matching property"
                );
            }
        }
        (Some(_), None) => {
            panic!("{path}: `required` present without `properties`");
        }
        _ => {}
    }

    // Recurse only into the places a Gemini Schema actually nests further
    // schema nodes: each property's schema, the array item schema, and each
    // `anyOf` branch. `properties`' own keys are argument *names*, not
    // schema keywords, so the map itself must never be walked generically.
    if let Some(serde_json::Value::Object(properties)) = map.get("properties") {
        for (key, nested) in properties {
            assert_gemini_compatible(nested, &format!("{path}.{key}"));
        }
    }

    if let Some(items) = map.get("items") {
        assert_gemini_compatible(items, &format!("{path}[]"));
    }

    if let Some(serde_json::Value::Array(any_of)) = map.get("anyOf") {
        for (index, branch) in any_of.iter().enumerate() {
            assert_gemini_compatible(branch, &format!("{path}/anyOf[{index}]"));
        }
    }
}

/// A JSON Schema shaped like a real-world MCP tool: `oneOf`, `$ref`/`$defs`,
/// `const`, a type array, `additionalProperties`, and `format: uri` — none
/// of which Gemini's Schema documents, and all of which Peach's own
/// `ToolCatalog` schemas mostly avoid (so this fixture is the only coverage
/// for them).
fn mcp_fixture_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "$defs": {
            "Coordinate": {
                "type": "object",
                "properties": {
                    "lat": { "type": "number" },
                    "lng": { "type": "number" }
                },
                "required": ["lat", "lng"]
            }
        },
        "properties": {
            "location": { "$ref": "#/$defs/Coordinate" },
            "status": { "const": "active" },
            "priority": { "type": ["integer", "null"] },
            "mode": {
                "oneOf": [
                    { "type": "string", "enum": ["fast", "slow"] },
                    { "type": "integer" }
                ]
            },
            "website": { "type": "string", "format": "uri" },
            "metadata": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            }
        },
        "required": ["location", "status"]
    })
}

#[test]
fn test_every_tool_catalog_schema_is_gemini_compatible() {
    for tool in ToolCatalog::iter() {
        let definition = tool.definition();
        let expected_name = definition.name.to_string();
        let decl = FunctionDeclaration::from(definition);
        assert_eq!(decl.name, expected_name);
        assert_gemini_compatible(&decl.parameters, &format!("tool:{expected_name}"));
    }
}

#[test]
fn test_mcp_fixture_schema_is_gemini_compatible_after_conversion() {
    let schema: Schema = mcp_fixture_schema()
        .try_into()
        .expect("fixture must be a valid JSON object schema");
    let tool_def = ToolDefinition::new("mcp_fixture")
        .description("A representative MCP tool schema")
        .input_schema(schema);

    let decl = FunctionDeclaration::from(tool_def);

    assert_gemini_compatible(&decl.parameters, "mcp_fixture");

    // Spot-check the specific constructs the fixture exists to cover: none
    // of the raw MCP keywords survive, and the ones with a direct Gemini
    // equivalent were converted rather than merely dropped.
    let params = decl.parameters.as_object().unwrap();
    assert!(!params.contains_key("$schema"));
    assert!(!params.contains_key("$defs"));

    let location = &decl.parameters["properties"]["location"];
    assert!(
        location.get("$ref").is_none(),
        "$ref must be inlined or removed, not sent to Gemini"
    );

    let status = decl.parameters["properties"]["status"].as_object().unwrap();
    assert!(!status.contains_key("const"), "const must become enum");
    assert_eq!(status["enum"], serde_json::json!(["active"]));

    let priority = decl.parameters["properties"]["priority"]
        .as_object()
        .unwrap();
    assert!(
        !priority["type"].is_array(),
        "type arrays must be resolved to a single type plus nullable"
    );
    assert_eq!(priority["nullable"], true);

    let mode = decl.parameters["properties"]["mode"].as_object().unwrap();
    assert!(!mode.contains_key("oneOf"), "oneOf must become anyOf");
    assert!(mode.contains_key("anyOf"));

    let website = decl.parameters["properties"]["website"]
        .as_object()
        .unwrap();
    assert!(
        !website.contains_key("format"),
        "uri is not a Gemini-supported string format"
    );

    let metadata = decl.parameters["properties"]["metadata"]
        .as_object()
        .unwrap();
    assert!(!metadata.contains_key("additionalProperties"));
}

#[test]
fn test_tool_catalog_gemini_declarations_snapshot() {
    let declarations = ToolCatalog::iter()
        .map(|tool| {
            let decl = FunctionDeclaration::from(tool.definition());
            serde_json::to_string_pretty(&serde_json::json!({
                "name": decl.name,
                "parameters": decl.parameters,
            }))
            .expect("Failed to serialize Gemini function declaration to JSON")
        })
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(declarations);
}

#[test]
fn test_mcp_fixture_gemini_declaration_snapshot() {
    let schema: Schema = mcp_fixture_schema().try_into().unwrap();
    let tool_def = ToolDefinition::new("mcp_fixture")
        .description("A representative MCP tool schema")
        .input_schema(schema);
    let decl = FunctionDeclaration::from(tool_def);

    let rendered = serde_json::to_string_pretty(&serde_json::json!({
        "name": decl.name,
        "parameters": decl.parameters,
    }))
    .unwrap();

    insta::assert_snapshot!(rendered);
}
