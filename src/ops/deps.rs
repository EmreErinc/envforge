// ─── Secret Dependency Mapping ─────────────────────────────
//
// `envforge deps KEY` — find all files that reference a given env var key.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use super::OpError;

/// A single reference to an env var key found in a file.
#[derive(Debug, Clone)]
pub struct DepReference {
    pub file: PathBuf,
    pub line: usize,
    pub context: String,
    pub ref_type: RefType,
}

/// The kind of file where the reference was found.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefType {
    EnvFile,
    SourceCode,
    Config,
    Schema,
    Shell,
    Other,
}

impl fmt::Display for RefType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefType::EnvFile => write!(f, "Project .env Files"),
            RefType::SourceCode => write!(f, "Source Code"),
            RefType::Config => write!(f, "Config Files"),
            RefType::Schema => write!(f, "Schema"),
            RefType::Shell => write!(f, "EnvForge Managed"),
            RefType::Other => write!(f, "Other"),
        }
    }
}

/// Directories to skip when walking.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "vendor",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
    ".next",
    ".nuxt",
];

/// Find all references to `key` across the project and EnvForge config.
pub fn find_dependencies(
    key: &str,
    project_dir: &Path,
    include_source: bool,
    managed_files: &[PathBuf],
) -> Result<Vec<DepReference>, OpError> {
    let mut refs = Vec::new();

    // 1. EnvForge managed shell files
    for path in managed_files {
        if path.exists() {
            scan_file_for_key(key, path, RefType::Shell, &mut refs);
        }
    }

    // 2. Project .env files
    scan_env_files(key, project_dir, &mut refs);

    // 3. Schema
    let schema_path = project_dir.join(".env.schema");
    if schema_path.exists() {
        scan_file_for_key(key, &schema_path, RefType::Schema, &mut refs);
    }

    // 4. Config files
    scan_config_files(key, project_dir, &mut refs);

    // 5. Source code (optional, slower)
    if include_source {
        scan_source_files(key, project_dir, &mut refs);
    }

    Ok(refs)
}

/// Group references by RefType for display.
pub fn group_by_type(refs: &[DepReference]) -> BTreeMap<RefType, Vec<&DepReference>> {
    let mut map: BTreeMap<RefType, Vec<&DepReference>> = BTreeMap::new();
    for r in refs {
        map.entry(r.ref_type.clone()).or_default().push(r);
    }
    map
}

// ─── Scanning helpers ─────────────────────────────────────

/// Scan a single file for lines containing the key.
fn scan_file_for_key(key: &str, path: &Path, ref_type: RefType, out: &mut Vec<DepReference>) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    for (i, line) in content.lines().enumerate() {
        if line.contains(key) {
            out.push(DepReference {
                file: path.to_path_buf(),
                line: i + 1,
                context: line.trim().to_string(),
                ref_type: ref_type.clone(),
            });
        }
    }
}

/// Scan .env* files in project root.
fn scan_env_files(key: &str, dir: &Path, out: &mut Vec<DepReference>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(".env") && !name_str.ends_with(".schema") && entry.path().is_file()
        {
            scan_file_for_key(key, &entry.path(), RefType::EnvFile, out);
        }
    }
}

/// Scan config files (docker-compose, terraform, k8s, Dockerfiles, GH workflows).
fn scan_config_files(key: &str, dir: &Path, out: &mut Vec<DepReference>) {
    // docker-compose files in root
    scan_glob_in_dir(key, dir, "docker-compose", ".yml", RefType::Config, out);
    scan_glob_in_dir(key, dir, "docker-compose", ".yaml", RefType::Config, out);

    // Dockerfiles in root
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("Dockerfile") && entry.path().is_file() {
            scan_file_for_key(key, &entry.path(), RefType::Config, out);
        }
    }

    // Terraform files in root
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if (name_str.ends_with(".tf") || name_str.ends_with(".tfvars")) && entry.path().is_file() {
            scan_file_for_key(key, &entry.path(), RefType::Config, out);
        }
    }

    // k8s / kubernetes subdirs
    for subdir_name in &["k8s", "kubernetes"] {
        let subdir = dir.join(subdir_name);
        if subdir.is_dir() {
            walk_and_scan(key, &subdir, &["yaml", "yml"], RefType::Config, out);
        }
    }

    // .github/workflows
    let workflows = dir.join(".github").join("workflows");
    if workflows.is_dir() {
        walk_and_scan(key, &workflows, &["yml", "yaml"], RefType::Config, out);
    }
}

/// Scan files matching a name prefix + suffix in a directory.
fn scan_glob_in_dir(
    key: &str,
    dir: &Path,
    prefix: &str,
    suffix: &str,
    ref_type: RefType,
    out: &mut Vec<DepReference>,
) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(prefix) && name_str.ends_with(suffix) && entry.path().is_file() {
            scan_file_for_key(key, &entry.path(), ref_type.clone(), out);
        }
    }
}

/// Walk a directory recursively scanning files with given extensions.
fn walk_and_scan(
    key: &str,
    dir: &Path,
    extensions: &[&str],
    ref_type: RefType,
    out: &mut Vec<DepReference>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !SKIP_DIRS.contains(&name_str.as_ref()) {
                walk_and_scan(key, &path, extensions, ref_type.clone(), out);
            }
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    scan_file_for_key(key, &path, ref_type.clone(), out);
                }
            }
        }
    }
}

/// Scan source files for language-specific env var access patterns.
fn scan_source_files(key: &str, dir: &Path, out: &mut Vec<DepReference>) {
    let patterns = build_source_patterns(key);
    let regexes: Vec<Regex> = patterns.iter().filter_map(|p| Regex::new(p).ok()).collect();

    walk_source_files(dir, &regexes, out);
}

fn build_source_patterns(key: &str) -> Vec<String> {
    let k = regex::escape(key);
    vec![
        // JavaScript / TypeScript
        format!(r"process\.env\.{}", k),
        format!(r#"process\.env\["{}"\]"#, k),
        // Python
        format!(r#"os\.environ\["{}"\]"#, k),
        format!(r#"os\.environ\.get\("{}""#, k),
        // Rust
        format!(r#"env::var\("{}""#, k),
        format!(r#"std::env::var\("{}""#, k),
        // Java
        format!(r#"System\.getenv\("{}""#, k),
        // Ruby
        format!(r#"ENV\["{}"\]"#, k),
        format!(r#"ENV\.fetch\("{}""#, k),
        // PHP / C
        format!(r#"getenv\("{}""#, k),
        format!(r#"\$_ENV\["{}"\]"#, k),
        // Go
        format!(r#"os\.Getenv\("{}""#, k),
        // Generic patterns
        format!(r"\$\{{{}\}}", k), // ${KEY} in configs
    ]
}

/// Source code file extensions to scan.
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "js", "ts", "jsx", "tsx", "py", "java", "go", "rb", "php", "c", "cpp", "h", "cs", "sh",
    "bash", "zsh", "toml", "yaml", "yml", "json", "xml", "tf", "hcl",
];

fn walk_source_files(dir: &Path, regexes: &[Regex], out: &mut Vec<DepReference>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !SKIP_DIRS.contains(&name_str.as_ref()) {
                walk_source_files(&path, regexes, out);
            }
        } else if path.is_file() {
            let ext_match = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| SOURCE_EXTENSIONS.contains(&e))
                .unwrap_or(false);
            if !ext_match {
                continue;
            }
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            for (i, line) in content.lines().enumerate() {
                for re in regexes {
                    if re.is_match(line) {
                        out.push(DepReference {
                            file: path.clone(),
                            line: i + 1,
                            context: line.trim().to_string(),
                            ref_type: RefType::SourceCode,
                        });
                        break; // one match per line is enough
                    }
                }
            }
        }
    }
}

// ─── Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_project(files: &[(&str, &str)]) -> TempDir {
        let tmp = TempDir::new().unwrap();
        for (name, content) in files {
            let path = tmp.path().join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, content).unwrap();
        }
        tmp
    }

    #[test]
    fn test_scan_finds_env_file_references() {
        let tmp = setup_project(&[
            (".env", "DB_PASSWORD=secret123\nPORT=8080\n"),
            (".env.production", "DB_PASSWORD=prod_secret\nPORT=443\n"),
        ]);

        let refs = find_dependencies("DB_PASSWORD", tmp.path(), false, &[]).unwrap();

        assert!(!refs.is_empty());
        assert!(refs.iter().all(|r| r.ref_type == RefType::EnvFile));
        assert_eq!(refs.len(), 2);
        assert!(refs
            .iter()
            .any(|r| r.context.contains("DB_PASSWORD=secret123")));
        assert!(refs
            .iter()
            .any(|r| r.context.contains("DB_PASSWORD=prod_secret")));
    }

    #[test]
    fn test_scan_finds_source_code_patterns() {
        let tmp = setup_project(&[
            (
                "src/db.rs",
                "fn connect() {\n    let pass = env::var(\"DB_PASSWORD\")?;\n}\n",
            ),
            (
                "src/config.py",
                "import os\ndb_pass = os.environ[\"DB_PASSWORD\"]\n",
            ),
            (
                "app/index.js",
                "const pass = process.env.DB_PASSWORD;\nconsole.log('ok');\n",
            ),
            (
                "main.go",
                "package main\nimport \"os\"\nfunc main() { os.Getenv(\"DB_PASSWORD\") }\n",
            ),
        ]);

        let refs = find_dependencies("DB_PASSWORD", tmp.path(), true, &[]).unwrap();

        let source_ref_count = refs
            .iter()
            .filter(|r| r.ref_type == RefType::SourceCode)
            .count();
        assert_eq!(source_ref_count, 4);
    }

    #[test]
    fn test_scan_skips_git_and_node_modules() {
        let tmp = setup_project(&[
            (".git/config", "DB_PASSWORD=leaked\n"),
            ("node_modules/pkg/index.js", "process.env.DB_PASSWORD\n"),
            ("src/app.js", "const x = process.env.DB_PASSWORD;\n"),
        ]);

        let refs = find_dependencies("DB_PASSWORD", tmp.path(), true, &[]).unwrap();

        // Should only find the src/app.js reference, not .git or node_modules
        let source_refs: Vec<_> = refs
            .iter()
            .filter(|r| r.ref_type == RefType::SourceCode)
            .collect();
        assert_eq!(source_refs.len(), 1);
        assert!(source_refs[0].file.to_string_lossy().contains("src/app.js"));
    }

    #[test]
    fn test_ref_type_classification() {
        let tmp = setup_project(&[
            (".env", "API_KEY=abc\n"),
            (
                ".env.schema",
                "[API_KEY]\ntype = \"string\"\nrequired = true\n",
            ),
            (
                "docker-compose.yml",
                "services:\n  web:\n    environment:\n      - API_KEY=${API_KEY}\n",
            ),
            ("src/main.rs", "let key = std::env::var(\"API_KEY\")?;\n"),
        ]);

        let refs = find_dependencies("API_KEY", tmp.path(), true, &[]).unwrap();

        let types: Vec<_> = refs.iter().map(|r| r.ref_type.clone()).collect();
        assert!(types.contains(&RefType::EnvFile));
        assert!(types.contains(&RefType::Schema));
        assert!(types.contains(&RefType::Config));
        assert!(types.contains(&RefType::SourceCode));
    }

    #[test]
    fn test_scan_finds_config_file_references() {
        let tmp = setup_project(&[
            (
                "docker-compose.yml",
                "services:\n  app:\n    environment:\n      - DB_HOST=${DB_HOST}\n",
            ),
            (
                ".github/workflows/ci.yml",
                "env:\n  DB_HOST: ${{ secrets.DB_HOST }}\n",
            ),
            ("Dockerfile", "ENV DB_HOST=${DB_HOST}\n"),
        ]);

        let refs = find_dependencies("DB_HOST", tmp.path(), false, &[]).unwrap();

        let config_ref_count = refs
            .iter()
            .filter(|r| r.ref_type == RefType::Config)
            .count();
        assert_eq!(config_ref_count, 3);
    }

    #[test]
    fn test_scan_managed_shell_files() {
        let tmp = setup_project(&[]);
        let shell_file = tmp.path().join("managed.sh");
        fs::write(
            &shell_file,
            "export DB_PASSWORD=\"secret\"\nexport PORT=8080\n",
        )
        .unwrap();

        let refs = find_dependencies("DB_PASSWORD", tmp.path(), false, &[shell_file]).unwrap();

        let shell_refs: Vec<_> = refs
            .iter()
            .filter(|r| r.ref_type == RefType::Shell)
            .collect();
        assert_eq!(shell_refs.len(), 1);
        assert!(shell_refs[0].context.contains("export DB_PASSWORD"));
    }

    #[test]
    fn test_group_by_type() {
        let refs = vec![
            DepReference {
                file: PathBuf::from(".env"),
                line: 1,
                context: "KEY=val".into(),
                ref_type: RefType::EnvFile,
            },
            DepReference {
                file: PathBuf::from("src/main.rs"),
                line: 10,
                context: "env::var(\"KEY\")".into(),
                ref_type: RefType::SourceCode,
            },
            DepReference {
                file: PathBuf::from(".env.prod"),
                line: 5,
                context: "KEY=prod".into(),
                ref_type: RefType::EnvFile,
            },
        ];
        let grouped = group_by_type(&refs);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[&RefType::EnvFile].len(), 2);
        assert_eq!(grouped[&RefType::SourceCode].len(), 1);
    }

    #[test]
    fn test_no_results_for_missing_key() {
        let tmp = setup_project(&[(".env", "PORT=8080\n"), ("src/main.rs", "fn main() {}\n")]);

        let refs = find_dependencies("NONEXISTENT_KEY", tmp.path(), true, &[]).unwrap();
        assert!(refs.is_empty());
    }
}
