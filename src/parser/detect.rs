use std::path::PathBuf;

use crate::model::{ParseError, Shell};

/// Detect the user's shell from the $SHELL environment variable.
pub fn detect_shell() -> Result<Shell, ParseError> {
    let shell_env = std::env::var("SHELL").map_err(|_| ParseError::ShellNotDetected)?;

    Ok(classify_shell(&shell_env))
}

/// Classify a shell path string into a Shell variant.
fn classify_shell(shell_path: &str) -> Shell {
    if shell_path.ends_with("/zsh") || shell_path == "zsh" {
        Shell::Zsh
    } else if shell_path.ends_with("/bash") || shell_path == "bash" {
        Shell::Bash
    } else {
        Shell::Unknown(shell_path.to_string())
    }
}

/// Get the list of config file names relevant to a shell type.
fn config_file_names(shell: &Shell) -> Vec<&'static str> {
    match shell {
        Shell::Zsh => vec![".zshrc", ".zprofile"],
        Shell::Bash => vec![".bashrc", ".bash_profile", ".profile"],
        Shell::Unknown(_) => vec![
            ".zshrc",
            ".zprofile",
            ".bashrc",
            ".bash_profile",
            ".profile",
        ],
    }
}

/// Scan for existing shell config files in the user's home directory.
///
/// Returns only files that actually exist on disk.
pub fn scan_config_files(shell: &Shell) -> Result<Vec<PathBuf>, ParseError> {
    let home = dirs::home_dir().ok_or(ParseError::HomeDirNotFound)?;
    let names = config_file_names(shell);

    let existing: Vec<PathBuf> = names
        .into_iter()
        .map(|name| home.join(name))
        .filter(|path| path.exists())
        .collect();

    Ok(existing)
}

/// Determine the default primary config file for a shell type.
pub fn default_primary_file(shell: &Shell) -> Result<PathBuf, ParseError> {
    let home = dirs::home_dir().ok_or(ParseError::HomeDirNotFound)?;

    let name = match shell {
        Shell::Zsh => ".zshrc",
        Shell::Bash => ".bashrc",
        Shell::Unknown(_) => ".bashrc",
    };

    Ok(home.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_shell_zsh() {
        assert_eq!(classify_shell("/bin/zsh"), Shell::Zsh);
        assert_eq!(classify_shell("/usr/local/bin/zsh"), Shell::Zsh);
        assert_eq!(classify_shell("zsh"), Shell::Zsh);
    }

    #[test]
    fn test_classify_shell_bash() {
        assert_eq!(classify_shell("/bin/bash"), Shell::Bash);
        assert_eq!(classify_shell("/usr/bin/bash"), Shell::Bash);
        assert_eq!(classify_shell("bash"), Shell::Bash);
    }

    #[test]
    fn test_classify_shell_unknown() {
        let result = classify_shell("/usr/bin/fish");
        assert!(matches!(result, Shell::Unknown(_)));
    }

    #[test]
    fn test_config_file_names_zsh() {
        let names = config_file_names(&Shell::Zsh);
        assert!(names.contains(&".zshrc"));
        assert!(names.contains(&".zprofile"));
    }

    #[test]
    fn test_config_file_names_bash() {
        let names = config_file_names(&Shell::Bash);
        assert!(names.contains(&".bashrc"));
        assert!(names.contains(&".bash_profile"));
        assert!(names.contains(&".profile"));
    }

    #[test]
    fn test_config_file_names_unknown_includes_all() {
        let names = config_file_names(&Shell::Unknown("fish".to_string()));
        assert!(names.len() >= 5);
    }
}
