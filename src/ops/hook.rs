use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::load_or_create_default;
use crate::ops::dotenv::parse_dotenv;
use crate::ops::encrypt::{decrypt_value, is_encrypted};
use crate::parser::parse_shell_file;

// ─── Hook Generation ────────────────────────────────────────

/// Generate a shell hook script for the given shell type.
///
/// The hook auto-loads environment variables when entering a directory
/// containing `.envforge.toml` or `.env.schema`, and auto-unloads when
/// leaving that directory.
pub fn generate_hook(shell: &str) -> Result<String, String> {
    match shell.to_lowercase().as_str() {
        "zsh" => Ok(generate_zsh_hook()),
        "bash" => Ok(generate_bash_hook()),
        "fish" => Ok(generate_fish_hook()),
        _ => Err(format!(
            "Unsupported shell '{}'. Supported: zsh, bash, fish",
            shell
        )),
    }
}

fn generate_zsh_hook() -> String {
    r##"_envforge_hook() {
  local envforge_dir=""
  local dir="$PWD"
  while [ "$dir" != "/" ]; do
    if [ -f "$dir/.envforge.toml" ] || [ -f "$dir/.env.schema" ]; then
      envforge_dir="$dir"
      break
    fi
    dir="$(dirname "$dir")"
  done

  if [ -n "$envforge_dir" ]; then
    if [ "$envforge_dir" != "$ENVFORGE_LOADED_DIR" ]; then
      # Unload previous if different dir
      if [ -n "$ENVFORGE_LOADED_DIR" ]; then
        _envforge_unload
      fi
      # Load new
      eval "$(envforge env --dir "$envforge_dir" 2>/dev/null)"
      export ENVFORGE_LOADED_DIR="$envforge_dir"
      echo "envforge: loaded from $envforge_dir" >&2
    fi
  elif [ -n "$ENVFORGE_LOADED_DIR" ]; then
    _envforge_unload
  fi
}

_envforge_unload() {
  if [ -n "$ENVFORGE_LOADED_DIR" ] && [ -f "$ENVFORGE_LOADED_DIR/.envforge-prev" ]; then
    eval "$(cat "$ENVFORGE_LOADED_DIR/.envforge-prev")"
    rm -f "$ENVFORGE_LOADED_DIR/.envforge-prev"
  fi
  unset ENVFORGE_LOADED_DIR
  echo "envforge: unloaded" >&2
}

typeset -ag chpwd_functions
if [[ -z "${chpwd_functions[(r)_envforge_hook]}" ]]; then
  chpwd_functions+=(_envforge_hook)
fi
_envforge_hook
"##
    .to_string()
}

fn generate_bash_hook() -> String {
    r##"_envforge_hook() {
  local envforge_dir=""
  local dir="$PWD"
  while [ "$dir" != "/" ]; do
    if [ -f "$dir/.envforge.toml" ] || [ -f "$dir/.env.schema" ]; then
      envforge_dir="$dir"
      break
    fi
    dir="$(dirname "$dir")"
  done

  if [ -n "$envforge_dir" ]; then
    if [ "$envforge_dir" != "$ENVFORGE_LOADED_DIR" ]; then
      # Unload previous if different dir
      if [ -n "$ENVFORGE_LOADED_DIR" ]; then
        _envforge_unload
      fi
      # Load new
      eval "$(envforge env --dir "$envforge_dir" 2>/dev/null)"
      export ENVFORGE_LOADED_DIR="$envforge_dir"
      echo "envforge: loaded from $envforge_dir" >&2
    fi
  elif [ -n "$ENVFORGE_LOADED_DIR" ]; then
    _envforge_unload
  fi
}

_envforge_unload() {
  if [ -n "$ENVFORGE_LOADED_DIR" ] && [ -f "$ENVFORGE_LOADED_DIR/.envforge-prev" ]; then
    eval "$(cat "$ENVFORGE_LOADED_DIR/.envforge-prev")"
    rm -f "$ENVFORGE_LOADED_DIR/.envforge-prev"
  fi
  unset ENVFORGE_LOADED_DIR
  echo "envforge: unloaded" >&2
}

PROMPT_COMMAND="_envforge_hook${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
_envforge_hook
"##
    .to_string()
}

fn generate_fish_hook() -> String {
    r##"function _envforge_hook --on-variable PWD
  set -l envforge_dir ""
  set -l dir $PWD
  while test "$dir" != "/"
    if test -f "$dir/.envforge.toml"; or test -f "$dir/.env.schema"
      set envforge_dir "$dir"
      break
    end
    set dir (dirname "$dir")
  end

  if test -n "$envforge_dir"
    if test "$envforge_dir" != "$ENVFORGE_LOADED_DIR"
      # Unload previous if different dir
      if test -n "$ENVFORGE_LOADED_DIR"
        _envforge_unload
      end
      # Load new
      envforge env --dir "$envforge_dir" 2>/dev/null | source
      set -gx ENVFORGE_LOADED_DIR "$envforge_dir"
      echo "envforge: loaded from $envforge_dir" >&2
    end
  else if test -n "$ENVFORGE_LOADED_DIR"
    _envforge_unload
  end
end

function _envforge_unload
  if test -n "$ENVFORGE_LOADED_DIR"; and test -f "$ENVFORGE_LOADED_DIR/.envforge-prev"
    cat "$ENVFORGE_LOADED_DIR/.envforge-prev" | source
    rm -f "$ENVFORGE_LOADED_DIR/.envforge-prev"
  end
  set -e ENVFORGE_LOADED_DIR
  echo "envforge: unloaded" >&2
end

_envforge_hook
"##
    .to_string()
}

// ─── Env Command ────────────────────────────────────────────

/// Output `export KEY='VALUE'` statements for the environment variables
/// in the given directory's project config.
///
/// Also saves previous values to `.envforge-prev` for clean unload.
pub fn cmd_env(dir: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = match dir {
        Some(d) => PathBuf::from(d),
        None => std::env::current_dir()?,
    };

    // Look for .envforge.toml project config
    let project_config_path = base_dir.join(".envforge.toml");
    let has_project_config = project_config_path.exists();
    let has_schema = base_dir.join(".env.schema").exists();

    if !has_project_config && !has_schema {
        return Err(format!(
            "No .envforge.toml or .env.schema found in {}",
            base_dir.display()
        )
        .into());
    }

    // Parse project config to get profile name
    let profile_name = if has_project_config {
        parse_project_config(&project_config_path)?
    } else {
        None
    };

    // Collect environment variables
    let env = collect_project_env(&base_dir, profile_name.as_deref())?;

    if env.is_empty() {
        return Ok(());
    }

    // Save previous values for unload
    save_prev_values(&base_dir, &env)?;

    // Output export statements to stdout
    let mut keys: Vec<&String> = env.keys().collect();
    keys.sort();
    for key in keys {
        let value = &env[key];
        println!("export {}={}", key, shell_escape(value));
    }

    Ok(())
}

/// Parse `.envforge.toml` project config.
///
/// Format:
/// ```toml
/// profile = "dev"
/// ```
fn parse_project_config(path: &Path) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let table: toml::Table = content.parse()?;
    Ok(table
        .get("profile")
        .and_then(|v| v.as_str())
        .map(String::from))
}

/// Collect environment variables for a project directory.
fn collect_project_env(
    base_dir: &Path,
    profile_name: Option<&str>,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut env: HashMap<String, String> = HashMap::new();

    // Strategy 1: Load from .env file in the directory
    let dotenv_path = base_dir.join(".env");
    if dotenv_path.exists() {
        let entries = parse_dotenv(&dotenv_path)?;
        for entry in entries {
            env.insert(entry.key, entry.value);
        }
    }

    // Strategy 2: Load from EnvForge profile if specified
    if let Some(profile) = profile_name {
        let config = load_or_create_default()?;
        if let Some(profile_entry) = config.profiles.entries.get(profile) {
            let profile_path = shellexpand(&profile_entry.file);
            if profile_path.exists() {
                merge_shell_file(&mut env, &profile_path);
            }
        }

        // Also load shared file
        let shared_path = shellexpand(&config.profiles.shared_file);
        if shared_path.exists() {
            merge_shell_file(&mut env, &shared_path);
        }
    }

    // Strategy 3: Load from .env.schema defaults
    let schema_path = base_dir.join(".env.schema");
    if schema_path.exists() {
        if let Ok(schema) = crate::ops::schema::parse_schema(&schema_path) {
            for (name, var) in &schema.variables {
                if !env.contains_key(name) {
                    if let Some(ref default) = var.default {
                        env.insert(name.clone(), default.clone());
                    }
                }
            }
        }
    }

    // Decrypt encrypted values
    let keys: Vec<String> = env.keys().cloned().collect();
    for key in &keys {
        if let Some(value) = env.get(key) {
            if is_encrypted(value) {
                if let Ok(decrypted) = decrypt_value(value) {
                    env.insert(key.clone(), decrypted);
                }
            }
        }
    }

    Ok(env)
}

/// Save previous shell values for the keys we're about to set.
/// This allows clean unloading.
fn save_prev_values(
    base_dir: &Path,
    env: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let prev_path = base_dir.join(".envforge-prev");
    let mut prev_content = String::new();

    for key in env.keys() {
        match std::env::var(key) {
            Ok(old_value) => {
                // Variable existed before -- restore it on unload
                prev_content.push_str(&format!(
                    "export {}={}\n",
                    key,
                    shell_escape(&old_value)
                ));
            }
            Err(_) => {
                // Variable did not exist -- unset it on unload
                prev_content.push_str(&format!("unset {}\n", key));
            }
        }
    }

    std::fs::write(prev_path, prev_content)?;
    Ok(())
}

/// Escape a value for safe use in shell `export KEY='VALUE'` statements.
///
/// Uses single quotes with `'\''` escape for values containing single quotes.
fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    // If the value contains no special characters, we can use it as-is with single quotes
    let escaped = value.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

fn merge_shell_file(env: &mut HashMap<String, String>, path: &Path) {
    if let Ok(sf) = parse_shell_file(path) {
        let entries = crate::ops::collect_all_entries(std::slice::from_ref(&sf));
        for entry in entries {
            if entry.location != crate::ops::EntryLocation::Commented {
                env.insert(entry.key, entry.value);
            }
        }
    }
}

fn shellexpand(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_escape_simple() {
        assert_eq!(shell_escape("hello"), "'hello'");
    }

    #[test]
    fn test_shell_escape_empty() {
        assert_eq!(shell_escape(""), "''");
    }

    #[test]
    fn test_shell_escape_single_quote() {
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_shell_escape_spaces() {
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn test_shell_escape_special_chars() {
        assert_eq!(shell_escape("$HOME/bin"), "'$HOME/bin'");
    }

    #[test]
    fn test_generate_hook_zsh() {
        let hook = generate_hook("zsh").unwrap();
        assert!(hook.contains("chpwd_functions"));
        assert!(hook.contains("_envforge_hook"));
        assert!(hook.contains("_envforge_unload"));
        assert!(hook.contains("ENVFORGE_LOADED_DIR"));
    }

    #[test]
    fn test_generate_hook_bash() {
        let hook = generate_hook("bash").unwrap();
        assert!(hook.contains("PROMPT_COMMAND"));
        assert!(hook.contains("_envforge_hook"));
        assert!(hook.contains("_envforge_unload"));
    }

    #[test]
    fn test_generate_hook_fish() {
        let hook = generate_hook("fish").unwrap();
        assert!(hook.contains("--on-variable PWD"));
        assert!(hook.contains("_envforge_hook"));
        assert!(hook.contains("_envforge_unload"));
    }

    #[test]
    fn test_generate_hook_unsupported() {
        assert!(generate_hook("powershell").is_err());
    }

    #[test]
    fn test_generate_hook_case_insensitive() {
        assert!(generate_hook("ZSH").is_ok());
        assert!(generate_hook("Bash").is_ok());
        assert!(generate_hook("FISH").is_ok());
    }

    #[test]
    fn test_parse_project_config() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".envforge.toml");
        std::fs::write(&config_path, "profile = \"dev\"\n").unwrap();

        let result = parse_project_config(&config_path).unwrap();
        assert_eq!(result, Some("dev".to_string()));
    }

    #[test]
    fn test_parse_project_config_no_profile() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(".envforge.toml");
        std::fs::write(&config_path, "# empty config\n").unwrap();

        let result = parse_project_config(&config_path).unwrap();
        assert_eq!(result, None);
    }
}
