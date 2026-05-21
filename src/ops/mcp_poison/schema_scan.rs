//! JSON Schema scanner. Flags overly-permissive env-mutation parameters
//! in MCP tool input schemas.

use serde_json::Value as JsonValue;

use crate::ops::mcp_poison::error::ScannerError;
use crate::ops::mcp_poison::finding::{PoisonFinding, Severity};

pub struct ToolSchema {
    pub tool_name: String,
    pub schema: JsonValue,
}

pub struct SchemaScanner;

const SENSITIVE_PARAM_NAMES: &[&str] = &[
    "env", "shell", "bash", "eval", "command", "cmd", "path", "argv", "args", "code",
];

impl SchemaScanner {
    pub const MAX_FINDINGS_PER_INPUT: usize = 100;

    pub fn scan(tool_schema: &ToolSchema) -> Result<Vec<PoisonFinding>, ScannerError> {
        let mut findings = Vec::new();
        walk_node(
            &tool_schema.schema,
            "",
            &tool_schema.tool_name,
            &mut findings,
        );
        Ok(findings)
    }
}

fn walk_node(node: &JsonValue, path: &str, tool_name: &str, findings: &mut Vec<PoisonFinding>) {
    if findings.len() >= SchemaScanner::MAX_FINDINGS_PER_INPUT {
        return;
    }
    match node {
        JsonValue::Object(map) => {
            // Recurse into `properties`
            if let Some(props) = map.get("properties").and_then(JsonValue::as_object) {
                for (name, sub) in props {
                    let new_path = if path.is_empty() {
                        name.clone()
                    } else {
                        format!("{path}.{name}")
                    };
                    inspect_property(name, sub, &new_path, tool_name, findings);
                    walk_node(sub, &new_path, tool_name, findings);
                }
            }
            // Recurse into oneOf/anyOf/allOf
            for combinator in ["oneOf", "anyOf", "allOf"] {
                if let Some(arr) = map.get(combinator).and_then(JsonValue::as_array) {
                    for (i, sub) in arr.iter().enumerate() {
                        let new_path = format!("{path}[{combinator}#{i}]");
                        walk_node(sub, &new_path, tool_name, findings);
                    }
                }
            }
        }
        JsonValue::Array(arr) => {
            for sub in arr {
                walk_node(sub, path, tool_name, findings);
            }
        }
        _ => {}
    }
}

fn inspect_property(
    name: &str,
    schema: &JsonValue,
    path: &str,
    tool_name: &str,
    findings: &mut Vec<PoisonFinding>,
) {
    let name_lc = name.to_lowercase();
    if !SENSITIVE_PARAM_NAMES.contains(&name_lc.as_str()) {
        return;
    }

    let obj = match schema.as_object() {
        Some(o) => o,
        None => return,
    };

    let type_is_string = obj
        .get("type")
        .and_then(JsonValue::as_str)
        .map(|t| t.eq_ignore_ascii_case("string"))
        .unwrap_or(false);

    if !type_is_string {
        return;
    }

    let has_enum = obj.contains_key("enum");
    let has_pattern = obj.contains_key("pattern");
    let has_format = obj.contains_key("format");

    if has_enum || has_pattern || has_format {
        return;
    }

    let (pattern_id, severity) = match name_lc.as_str() {
        "env" => ("schema_env_broad", Severity::High),
        "shell" => ("schema_shell_broad", Severity::High),
        "bash" => ("schema_shell_broad", Severity::High),
        "command" | "cmd" => ("schema_command_broad", Severity::High),
        "eval" => ("schema_eval_broad", Severity::Critical),
        "path" => ("schema_path_broad", Severity::Medium),
        "argv" | "args" => ("schema_command_broad", Severity::High),
        "code" => ("schema_eval_broad", Severity::Critical),
        _ => return,
    };

    let span = (0usize, path.len());
    findings.push(PoisonFinding::new(
        tool_name, pattern_id, severity, span, path, 0, false,
    ));
}
