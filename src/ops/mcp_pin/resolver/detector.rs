//! Pure dispatch from `McpConfigFragment` → `PackageManager`.
//!
//! No I/O. Deterministic.

use std::path::PathBuf;

use super::fragment::McpConfigFragment;
use super::ResolverError;
use crate::ops::mcp_pin::types::{PackageManager, Transport};

pub struct PackageManagerDetector;

impl PackageManagerDetector {
    pub fn detect(fragment: &McpConfigFragment) -> Result<PackageManager, ResolverError> {
        let transport = fragment.effective_transport();
        let has_command = fragment.command.is_some();
        let has_url = fragment.url.is_some();

        // Remote transports
        if matches!(transport, Transport::Sse | Transport::Http) {
            let url = fragment
                .url
                .clone()
                .ok_or_else(|| ResolverError::EmptyConfig {
                    name: fragment.name.clone(),
                })?;
            if has_command {
                return Err(ResolverError::AmbiguousConfig {
                    name: fragment.name.clone(),
                    reason: "remote transport must not specify command".into(),
                });
            }
            return Ok(match transport {
                Transport::Sse => PackageManager::RemoteSse { url },
                Transport::Http => PackageManager::RemoteHttp { url },
                Transport::Stdio => unreachable!(),
            });
        }

        // Stdio transport
        if !has_command {
            if has_url {
                return Err(ResolverError::AmbiguousConfig {
                    name: fragment.name.clone(),
                    reason: "url present but transport is stdio".into(),
                });
            }
            return Err(ResolverError::EmptyConfig {
                name: fragment.name.clone(),
            });
        }
        if has_url {
            return Err(ResolverError::AmbiguousConfig {
                name: fragment.name.clone(),
                reason: "both command and url present".into(),
            });
        }

        let cmd = fragment.command.as_deref().unwrap();
        let args = fragment.args.clone().unwrap_or_default();

        match cmd {
            "npx" => Self::dispatch_npx(&args, fragment),
            "uvx" => Self::dispatch_uvx(&args, fragment),
            "pip" | "pip3" => Self::dispatch_pip(&args, fragment),
            "python" | "python3" => Self::dispatch_python(&args, fragment),
            other => Self::dispatch_bare(other, fragment),
        }
    }

    fn dispatch_npx(
        args: &[String],
        fragment: &McpConfigFragment,
    ) -> Result<PackageManager, ResolverError> {
        // Find first non-flag argument as the package spec; skip `-y`, `--yes`,
        // and `-p <pkg>` resolves to the package itself.
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            if a == "-y" || a == "--yes" {
                i += 1;
                continue;
            }
            if a == "-p" || a == "--package" {
                if i + 1 < args.len() {
                    return Ok(parse_npm_pkg_spec(&args[i + 1]));
                }
                return Err(ResolverError::UnknownPackageManager {
                    command: format!("npx with dangling {a}"),
                });
            }
            if a.starts_with('-') {
                i += 1;
                continue;
            }
            return Ok(parse_npm_pkg_spec(a));
        }
        Err(ResolverError::UnknownPackageManager {
            command: format!("npx with no package arg (server {})", fragment.name),
        })
    }

    fn dispatch_uvx(
        args: &[String],
        fragment: &McpConfigFragment,
    ) -> Result<PackageManager, ResolverError> {
        let first = args.iter().find(|a| !a.starts_with('-')).ok_or_else(|| {
            ResolverError::UnknownPackageManager {
                command: format!("uvx with no package (server {})", fragment.name),
            }
        })?;
        let (pkg, ver) = split_pkg_version(first);
        Ok(PackageManager::Uvx { pkg, ver })
    }

    fn dispatch_pip(
        args: &[String],
        fragment: &McpConfigFragment,
    ) -> Result<PackageManager, ResolverError> {
        // Skip pip subcommand (`install`, `run`) and find first non-flag arg.
        let mut iter = args.iter();
        let _subcommand = iter.next();
        for a in iter {
            if a.starts_with('-') {
                continue;
            }
            let (pkg, ver) = split_pip_pkg_version(a);
            return Ok(PackageManager::Pip { pkg, ver });
        }
        Err(ResolverError::UnknownPackageManager {
            command: format!("pip with no package (server {})", fragment.name),
        })
    }

    fn dispatch_python(
        args: &[String],
        fragment: &McpConfigFragment,
    ) -> Result<PackageManager, ResolverError> {
        // `python -m <module>` form
        let mut iter = args.iter();
        while let Some(a) = iter.next() {
            if a == "-m" {
                if let Some(module) = iter.next() {
                    return Ok(PackageManager::PythonModule {
                        module: module.clone(),
                    });
                }
            }
        }
        Err(ResolverError::UnknownPackageManager {
            command: format!("python without -m (server {})", fragment.name),
        })
    }

    fn dispatch_bare(
        cmd: &str,
        _fragment: &McpConfigFragment,
    ) -> Result<PackageManager, ResolverError> {
        Ok(PackageManager::Bare {
            path: PathBuf::from(cmd),
        })
    }
}

/// Parse an npm package spec like `@scope/pkg@1.2.3` or `pkg@1.0.0` or `pkg`.
fn parse_npm_pkg_spec(spec: &str) -> PackageManager {
    let (pkg, ver) = split_pkg_version(spec);
    PackageManager::Npm { pkg, ver }
}

/// Split pip-style `pkg==version` (or `pkg`).
fn split_pip_pkg_version(spec: &str) -> (String, Option<String>) {
    if let Some((pkg, ver)) = spec.split_once("==") {
        if ver.is_empty() {
            return (pkg.to_string(), None);
        }
        return (pkg.to_string(), Some(ver.to_string()));
    }
    if let Some((pkg, ver)) = spec.split_once(">=") {
        return (pkg.to_string(), Some(ver.to_string()));
    }
    (spec.to_string(), None)
}

/// Split `pkg@version` honoring scoped names (`@scope/name@version`).
fn split_pkg_version(spec: &str) -> (String, Option<String>) {
    let bytes = spec.as_bytes();
    // For scoped packages the leading '@' is part of the name; find the
    // *last* '@' after the first character.
    let start = if bytes.first() == Some(&b'@') { 1 } else { 0 };
    if let Some(idx) = spec[start..].rfind('@') {
        let pkg_end = start + idx;
        let pkg = spec[..pkg_end].to_string();
        let ver = spec[pkg_end + 1..].to_string();
        if ver.is_empty() {
            (pkg, None)
        } else {
            (pkg, Some(ver))
        }
    } else {
        (spec.to_string(), None)
    }
}
