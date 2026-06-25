//! PATH lookup + binary hashing (composes `BinaryHasher`).

use std::path::{Path, PathBuf};

use super::ResolverError;
use crate::ops::mcp_pin::hasher::{BinaryHasher, HashedBinary};

pub struct BinaryPathResolver;

impl BinaryPathResolver {
    /// PATH lookup. Returns the first executable match in `PATH` order.
    pub fn resolve_path(command: &str) -> Result<PathBuf, ResolverError> {
        let p = Path::new(command);
        if p.is_absolute() {
            if p.exists() {
                return Ok(p.to_path_buf());
            }
            return Err(ResolverError::CommandNotFound {
                cmd: command.to_string(),
            });
        }

        let path_var = std::env::var_os("PATH").ok_or_else(|| ResolverError::CommandNotFound {
            cmd: command.to_string(),
        })?;
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(command);
            if candidate.is_file() && is_executable(&candidate) {
                return Ok(candidate);
            }
            #[cfg(windows)]
            {
                for ext in ["exe", "cmd", "bat"] {
                    let mut with_ext = candidate.clone();
                    with_ext.set_extension(ext);
                    if with_ext.is_file() {
                        return Ok(with_ext);
                    }
                }
            }
        }

        Err(ResolverError::CommandNotFound {
            cmd: command.to_string(),
        })
    }

    /// PATH lookup + `BinaryHasher::hash_binary`.
    pub fn hash_binary_command(command: &str) -> Result<HashedBinary, ResolverError> {
        let path = Self::resolve_path(command)?;
        BinaryHasher::hash_binary(&path).map_err(ResolverError::from)
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}
