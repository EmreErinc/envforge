/// Generate JSON Schema for .env.schema format.
pub fn generate_json_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://github.com/emreerinc/envforge/raw/main/.env.schema.json",
        "title": "EnvForge Environment Schema",
        "description": "Schema for .env.schema files used by EnvForge",
        "type": "object",
        "additionalProperties": {
            "$ref": "#/$defs/variable"
        },
        "$defs": {
            "variable": {
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "enum": ["string", "number", "bool", "url", "email", "enum", "regex", "port"]
                    },
                    "required": { "type": "boolean" },
                    "default": { "type": "string" },
                    "description": { "type": "string" },
                    "example": { "type": "string" },
                    "sensitive": { "type": "boolean" },
                    "pattern": { "type": "string" },
                    "values": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "min": { "type": "number" },
                    "max": { "type": "number" }
                },
                "additionalProperties": {
                    "$ref": "#/$defs/env-override"
                }
            },
            "env-override": {
                "type": "object",
                "properties": {
                    "required": { "type": "boolean" },
                    "default": { "type": "string" },
                    "description": { "type": "string" },
                    "pattern": { "type": "string" },
                    "values": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "min": { "type": "number" },
                    "max": { "type": "number" }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_schema_valid() {
        let schema = generate_json_schema();

        // Verify it's a valid JSON object
        assert!(schema.is_object(), "Schema should be a JSON object");

        // Verify $schema field
        assert_eq!(
            schema["$schema"], "https://json-schema.org/draft/2020-12/schema",
            "Should reference Draft 2020-12"
        );

        // Verify $defs exists and contains expected definitions
        let defs = &schema["$defs"];
        assert!(defs.is_object(), "$defs should be an object");
        assert!(
            defs.get("variable").is_some(),
            "$defs should contain 'variable'"
        );
        assert!(
            defs.get("env-override").is_some(),
            "$defs should contain 'env-override'"
        );

        // Verify variable type enum values
        let type_enum = &defs["variable"]["properties"]["type"]["enum"];
        assert!(type_enum.is_array(), "type should have an enum array");
        let types: Vec<&str> = type_enum
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(types.contains(&"string"));
        assert!(types.contains(&"port"));
        assert!(types.contains(&"email"));

        // Verify top-level additionalProperties references variable def
        assert_eq!(schema["additionalProperties"]["$ref"], "#/$defs/variable");
    }
}
