use serde_json::{json, Value};

use crate::SPECIAL_FORMS;

pub fn core_schema() -> Value {
    let reserved: Vec<&str> = SPECIAL_FORMS
        .iter()
        .flat_map(|form| std::iter::once(form.name).chain(form.aliases.iter().copied()))
        .collect();

    let special_forms: Vec<Value> = SPECIAL_FORMS
        .iter()
        .map(|form| {
            let names: Vec<&str> = std::iter::once(form.name)
                .chain(form.aliases.iter().copied())
                .collect();
            let max_items = form.max_args.map(|value| value + 1);
            let mut schema = json!({
                "type": "array",
                "minItems": form.min_args + 1,
                "prefixItems": [{ "enum": names }],
                "items": { "$ref": "#/$defs/node" },
                "description": form.summary,
            });
            if let Some(max_items) = max_items {
                schema["maxItems"] = json!(max_items);
            }
            schema
        })
        .collect();

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://jisp.dev/schema/core.json",
        "title": "Jisp canonical JSON AST",
        "type": "array",
        "items": { "$ref": "#/$defs/topLevel" },
        "$defs": {
            "symbol": {
                "type": "string",
                "minLength": 1,
                "pattern": "^\\S+$"
            },
            "node": {
                "anyOf": [
                    { "type": "null" },
                    { "type": "boolean" },
                    { "type": "integer" },
                    { "type": "number" },
                    { "$ref": "#/$defs/symbol" },
                    { "$ref": "#/$defs/form" }
                ]
            },
            "form": {
                "anyOf": [
                    { "type": "array", "minItems": 1, "prefixItems": [{ "const": "str" }] },
                    { "type": "array", "minItems": 1, "prefixItems": [{ "const": "str.lines" }] },
                    { "oneOf": special_forms },
                    {
                        "type": "array",
                        "minItems": 1,
                        "prefixItems": [{
                            "anyOf": [
                                {
                                    "$ref": "#/$defs/symbol",
                                    "not": { "enum": reserved }
                                },
                                { "$ref": "#/$defs/form" }
                            ]
                        }],
                        "items": { "$ref": "#/$defs/node" }
                    }
                ]
            },
            "topLevel": {
                "anyOf": [
                    {
                        "type": "array",
                        "minItems": 2,
                        "prefixItems": [{ "enum": ["def", "defn", "export", "import", "macro-import", "type", "component", "ui.app"] }]
                    }
                ]
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_mentions_core_forms() {
        let text = serde_json::to_string(&core_schema()).unwrap();
        assert!(text.contains("\"case\""));
        assert!(text.contains("\"defn\""));
        assert!(text.contains("\"export\""));
        assert!(text.contains("\"macro-import\""));
        assert!(text.contains("\"str.lines\""));
    }
}
