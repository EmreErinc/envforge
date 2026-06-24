//! Resolve a [`ProjectConfig`]'s declared environments into the concrete set of
//! env-file paths the LSP recognizes (Epic 1: Project Manifest Foundation).
//!
//! The project config (`.envforge.project.toml` / `.yaml` / `.json`, parsed by
//! [`super::config`]) is the source of truth for *which* env files belong to a
//! project. This module turns its `environments` list into absolute paths,
//! dropping any entry whose path would escape the project root — recognition
//! must never reach outside the workspace (NFR6 / scoping rule).
//!
//! Pure logic, no I/O: callers pass an already-loaded `ProjectConfig` and the
//! project root. The LSP layer consults [`ResolvedEnvSet::recognizes`] as the
//! single source of truth for env-file recognition; when no project config is
//! present it falls back to conventional `.env*` recognition (Story 1.5).

use std::path::{Component, Path, PathBuf};

use super::config::ProjectConfig;

/// A single project environment resolved to a concrete env-file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEnv {
    /// Environment name as declared in the config (e.g. `development`, `production`).
    pub name: String,
    /// Absolute, lexically-normalized path to this environment's env file.
    pub path: PathBuf,
    /// Whether this is the project's active environment.
    pub is_active: bool,
}

/// The full set of env files a project declares, resolved to concrete paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEnvSet {
    /// Project root the paths were resolved against (lexically normalized).
    pub root: PathBuf,
    /// One entry per declared environment whose path stays within `root`,
    /// de-duplicated by resolved path (first declaration wins).
    pub envs: Vec<ResolvedEnv>,
}

impl ResolvedEnvSet {
    /// True if `path` is one of the recognized env files.
    ///
    /// Comparison is on lexically-normalized absolute paths so it works for
    /// files that do not yet exist on disk (no canonicalization required).
    pub fn recognizes(&self, path: &Path) -> bool {
        let target = lexical_normalize(path);
        self.envs.iter().any(|e| e.path == target)
    }

    /// Iterator over the recognized env-file paths.
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.envs.iter().map(|e| e.path.as_path())
    }

    /// The environment name a recognized file belongs to, if any. Used to
    /// scope cross-environment diagnostics to "the environment being edited".
    pub fn env_name_for(&self, path: &Path) -> Option<&str> {
        let target = lexical_normalize(path);
        self.envs
            .iter()
            .find(|e| e.path == target)
            .map(|e| e.name.as_str())
    }

    /// Whether the set contains no recognized env files.
    pub fn is_empty(&self) -> bool {
        self.envs.is_empty()
    }
}

/// Resolve every environment in `config` to its concrete env-file path under
/// `project_root`.
///
/// - `project_root` is lexically normalized; relative roots are joined onto the
///   current directory is *not* performed here — callers pass the project root
///   from [`super::config::DetectedConfig`], which is already absolute.
/// - Each `env_file` is joined onto the root and lexically normalized. Entries
///   whose normalized path escapes the root (via `..` or an absolute
///   `env_file`) are dropped — recognition never reaches outside the workspace.
/// - Duplicate resolved paths are collapsed (first declaration wins).
pub fn resolve_env_set(config: &ProjectConfig, project_root: &Path) -> ResolvedEnvSet {
    let root = lexical_normalize(project_root);
    let active = config.project.active_environment.as_str();

    let mut envs: Vec<ResolvedEnv> = Vec::new();
    for env in &config.environments {
        let Some(path) = resolve_within_root(&root, &env.env_file) else {
            // Path escapes the project root (traversal or absolute) — skip it.
            continue;
        };
        if envs.iter().any(|e| e.path == path) {
            continue; // de-dup by resolved path
        }
        envs.push(ResolvedEnv {
            name: env.name.clone(),
            path,
            is_active: env.name == active,
        });
    }

    ResolvedEnvSet { root, envs }
}

/// Join `rel` onto `root` and lexically normalize, returning `None` if the
/// result escapes `root` (parent traversal above root, or `rel` absolute).
fn resolve_within_root(root: &Path, rel: &Path) -> Option<PathBuf> {
    if rel.is_absolute() {
        return None;
    }
    let joined = lexical_normalize(&root.join(rel));
    joined.starts_with(root).then_some(joined)
}

/// Collapse `.` and `..` components lexically (no filesystem access).
///
/// A leading `..` that would rise above the path's anchor is dropped, which —
/// combined with the `starts_with(root)` check in [`resolve_within_root`] —
/// ensures escapes are rejected rather than silently rebased.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s),
        }
    }
    out
}
