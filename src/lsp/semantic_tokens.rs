use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
};

use crate::ops::dotenv::is_sensitive_key;
use crate::ops::schema::EnvSchema;

use super::document::{EnvDocEntry, EnvLineType};

/// Token-type legend declared in `initialize`. Indices in this slice
/// correspond to the `token_type` field on each emitted `SemanticToken`.
pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::VARIABLE, // 0 — env var keys
    SemanticTokenType::STRING,   // 1 — env var values
    SemanticTokenType::COMMENT,  // 2 — comments
];

/// Modifier legend declared in `initialize`. Bit indices in this slice
/// correspond to the `token_modifiers_bitset` field. We piggyback on
/// `READONLY` as the "sensitive / secret" marker: in practice every
/// VS Code / JetBrains theme tints `readonly` distinctly, which is the
/// visual cue we want for secret keys/values.
pub const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::READONLY, // bit 0 — used for sensitive keys/values
];

const TYPE_VARIABLE: u32 = 0;
const TYPE_STRING: u32 = 1;
const TYPE_COMMENT: u32 = 2;

const MOD_SENSITIVE: u32 = 1 << 0;

#[derive(Debug, Clone)]
struct RawToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    modifiers: u32,
}

pub fn compute_semantic_tokens(
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
) -> SemanticTokens {
    let mut raws: Vec<RawToken> = Vec::new();

    for entry in entries {
        match entry.line_type {
            EnvLineType::Comment => {
                let start = entry.key_range.start.character;
                let end = entry.key_range.end.character;
                if end > start {
                    raws.push(RawToken {
                        line: entry.line,
                        start,
                        length: end - start,
                        token_type: TYPE_COMMENT,
                        modifiers: 0,
                    });
                }
            }
            EnvLineType::EnvVar => {
                let sensitive = schema
                    .and_then(|s| s.variables.get(&entry.key))
                    .map(|v| v.sensitive)
                    .unwrap_or(false)
                    || is_sensitive_key(&entry.key);
                let mods = if sensitive { MOD_SENSITIVE } else { 0 };

                let key_len = entry.key_range.end.character - entry.key_range.start.character;
                if key_len > 0 {
                    raws.push(RawToken {
                        line: entry.line,
                        start: entry.key_range.start.character,
                        length: key_len,
                        token_type: TYPE_VARIABLE,
                        modifiers: mods,
                    });
                }

                if !sensitive {
                    let val_len =
                        entry.value_range.end.character - entry.value_range.start.character;
                    if val_len > 0 {
                        raws.push(RawToken {
                            line: entry.line,
                            start: entry.value_range.start.character,
                            length: val_len,
                            token_type: TYPE_STRING,
                            modifiers: 0,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    raws.sort_by_key(|a| (a.line, a.start));

    let mut data = Vec::with_capacity(raws.len());
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;
    for raw in raws {
        let delta_line = raw.line - prev_line;
        let delta_start = if delta_line == 0 {
            raw.start - prev_start
        } else {
            raw.start
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: raw.length,
            token_type: raw.token_type,
            token_modifiers_bitset: raw.modifiers,
        });
        prev_line = raw.line;
        prev_start = raw.start;
    }

    SemanticTokens {
        result_id: None,
        data,
    }
}
