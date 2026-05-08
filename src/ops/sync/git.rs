use super::model::*;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Trait for Git operations — enables mocking in tests.
pub trait GitOps {
    fn check_available(&self) -> Result<GitVersion, SyncError>;
    fn init(&self, branch: &str) -> Result<(), SyncError>;
    fn clone_repo(url: &str, target: &Path) -> Result<(), SyncError>
    where
        Self: Sized;
    fn add(&self, files: &[&str]) -> Result<(), SyncError>;
    fn add_all(&self) -> Result<(), SyncError>;
    fn commit(&self, message: &str) -> Result<String, SyncError>;
    fn push(&self) -> Result<PushResult, SyncError>;
    fn pull(&self) -> Result<PullResult, SyncError>;
    fn status(&self) -> Result<Vec<FileStatus>, SyncError>;
    fn log(&self, limit: usize) -> Result<Vec<GitCommitInfo>, SyncError>;
    fn show(&self, commit: &str, file: &str) -> Result<String, SyncError>;
    fn remote_url(&self) -> Result<Option<String>, SyncError>;
    fn has_changes(&self) -> Result<bool, SyncError>;
}

/// Git binary wrapper using std::process::Command.
pub struct GitCommandRunner {
    repo_path: PathBuf,
}

impl GitCommandRunner {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    /// Ensure git user.name and user.email are configured for this repo.
    /// Sets repo-local defaults if not already set globally.
    pub fn ensure_user_config(&self) -> Result<(), SyncError> {
        // Check if user.name is set (global or local)
        if self.run_git(&["config", "user.name"]).is_err() {
            self.run_git(&["config", "user.name", "envforge"])?;
        }
        if self.run_git(&["config", "user.email"]).is_err() {
            self.run_git(&["config", "user.email", "envforge@localhost"])?;
        }
        Ok(())
    }

    /// Run a git command in the repo directory and capture output.
    fn run_git(&self, args: &[&str]) -> Result<String, SyncError> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.repo_path);
        cmd.args(args);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SyncError::GitNotFound
            } else {
                SyncError::GitCommandFailed {
                    command: format!("git {}", args.join(" ")),
                    stderr: e.to_string(),
                }
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let command = format!("git {}", args.join(" "));

            // Classify error by stderr content
            if stderr.contains("Authentication")
                || stderr.contains("Permission denied")
                || stderr.contains("could not read Username")
            {
                Err(SyncError::AuthFailed)
            } else if stderr.contains("CONFLICT") || stderr.contains("Merge conflict") {
                let files = parse_conflict_files(&stderr);
                Err(SyncError::PullConflict { files })
            } else if stderr.contains("rejected")
                || stderr.contains("non-fast-forward")
                || stderr.contains("failed to push")
            {
                Err(SyncError::PushRejected)
            } else {
                Err(SyncError::GitCommandFailed { command, stderr })
            }
        }
    }

    /// Run a git command without -C (for operations on paths that don't exist yet).
    fn run_git_raw(args: &[&str]) -> Result<String, SyncError> {
        let output = Command::new("git").args(args).output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SyncError::GitNotFound
            } else {
                SyncError::GitCommandFailed {
                    command: format!("git {}", args.join(" ")),
                    stderr: e.to_string(),
                }
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(SyncError::GitCommandFailed {
                command: format!("git {}", args.join(" ")),
                stderr,
            })
        }
    }
}

impl GitOps for GitCommandRunner {
    fn check_available(&self) -> Result<GitVersion, SyncError> {
        let output = Command::new("git")
            .arg("--version")
            .output()
            .map_err(|_| SyncError::GitNotFound)?;

        if !output.status.success() {
            return Err(SyncError::GitNotFound);
        }

        let version_str = String::from_utf8_lossy(&output.stdout);
        let version = parse_git_version(&version_str)?;

        if !version.meets_minimum() {
            return Err(SyncError::GitVersionTooOld {
                found: version.to_string(),
                required: GitVersion::MINIMUM.to_string(),
            });
        }

        Ok(version)
    }

    fn init(&self, branch: &str) -> Result<(), SyncError> {
        std::fs::create_dir_all(&self.repo_path).map_err(|e| SyncError::IoError {
            path: self.repo_path.clone(),
            source: e,
        })?;
        self.run_git(&["init", "-b", branch])?;
        Ok(())
    }

    fn clone_repo(url: &str, target: &Path) -> Result<(), SyncError> {
        validate_remote_url(url)?;
        // Disable git-remote-ext (RCE vector) and dumb file/local protocol unless
        // explicitly allowlisted via validate_remote_url.
        Self::run_git_raw(&[
            "-c",
            "protocol.ext.allow=never",
            "-c",
            "protocol.file.allow=user",
            "clone",
            "--",
            url,
            &target.to_string_lossy(),
        ])?;
        Ok(())
    }

    fn add(&self, files: &[&str]) -> Result<(), SyncError> {
        let mut args = vec!["add"];
        args.extend_from_slice(files);
        self.run_git(&args)?;
        Ok(())
    }

    fn add_all(&self) -> Result<(), SyncError> {
        self.run_git(&["add", "-A"])?;
        Ok(())
    }

    fn commit(&self, message: &str) -> Result<String, SyncError> {
        let output = self.run_git(&["commit", "-m", message])?;

        // Parse commit hash from output (first line usually contains it)
        for line in output.lines() {
            // Format: "[main abc1234] commit message"
            if let Some(start) = line.find(' ') {
                let after_bracket = &line[1..start];
                // This is the branch name, hash is after the next space
                if let Some(hash_part) = line.get(start + 1..) {
                    if let Some(end) = hash_part.find(']') {
                        return Ok(hash_part[..end].to_string());
                    }
                }
                // Fallback: return trimmed first line
                return Ok(after_bracket.to_string());
            }
        }

        Ok(String::new())
    }

    fn push(&self) -> Result<PushResult, SyncError> {
        // Check if remote exists first
        if self.remote_url()?.is_none() {
            return Ok(PushResult::NoRemote);
        }

        match self.run_git(&["push", "origin", "main"]) {
            Ok(_) => Ok(PushResult::Success),
            Err(SyncError::PushRejected) => Ok(PushResult::Rejected),
            Err(e) => Err(e),
        }
    }

    fn pull(&self) -> Result<PullResult, SyncError> {
        match self.run_git(&["pull", "origin", "main"]) {
            Ok(output) => {
                if output.contains("Already up to date") {
                    Ok(PullResult::UpToDate)
                } else {
                    Ok(PullResult::Updated)
                }
            }
            Err(SyncError::PullConflict { files }) => Ok(PullResult::Conflict { files }),
            Err(e) => Err(e),
        }
    }

    fn status(&self) -> Result<Vec<FileStatus>, SyncError> {
        let output = self.run_git(&["status", "--porcelain"])?;
        Ok(parse_git_status(&output))
    }

    fn log(&self, limit: usize) -> Result<Vec<GitCommitInfo>, SyncError> {
        let limit_str = format!("-{}", limit);
        let output = self.run_git(&[
            "log",
            "--format=%H|%h|%aI|%s|%an",
            &limit_str,
            "--no-merges",
        ])?;
        Ok(parse_git_log(&output))
    }

    fn show(&self, commit: &str, file: &str) -> Result<String, SyncError> {
        let spec = format!("{}:{}", commit, file);
        self.run_git(&["show", &spec])
    }

    fn remote_url(&self) -> Result<Option<String>, SyncError> {
        match self.run_git(&["remote", "get-url", "origin"]) {
            Ok(url) => {
                let trimmed = url.trim().to_string();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed))
                }
            }
            Err(SyncError::GitCommandFailed { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn has_changes(&self) -> Result<bool, SyncError> {
        let status = self.status()?;
        Ok(!status.is_empty())
    }
}

impl GitCommandRunner {
    /// Run `git verify-commit <ref>` and require a Good/Trusted signature.
    /// Used when `verify_signatures` is enabled to fail closed if a remote
    /// pulled commit was not signed by a trusted key.
    pub fn verify_commit(&self, commit: &str) -> Result<(), SyncError> {
        if commit.is_empty() || commit.starts_with('-') {
            return Err(SyncError::GitCommandFailed {
                command: "verify-commit".to_string(),
                stderr: "invalid commit ref".to_string(),
            });
        }
        // `--raw` would be machine-friendlier but isn't universally supported;
        // on failure git returns non-zero, which run_git already maps to
        // GitCommandFailed.
        self.run_git(&["verify-commit", "--", commit]).map(|_| ())
    }
}

// ─── URL Validation ──────────────────────────────────────────

/// Reject remote URLs that could trigger arbitrary command execution
/// (`ext::`), local file disclosure (`file://`), or argument injection
/// (leading `-`). Only allow `https://`, `http://`, `ssh://`, `git://`,
/// or scp-like `user@host:path`.
pub fn validate_remote_url(url: &str) -> Result<(), SyncError> {
    let trimmed = url.trim();
    let bad = |msg: &str| SyncError::GitCommandFailed {
        command: "clone".to_string(),
        stderr: format!("invalid remote URL ({}): {}", msg, url),
    };

    if trimmed.is_empty() {
        return Err(bad("empty"));
    }
    if trimmed.starts_with('-') {
        return Err(bad("leading dash"));
    }
    if trimmed.contains(['\n', '\r', '\0']) {
        return Err(bad("control character"));
    }

    let lower = trimmed.to_ascii_lowercase();
    // Block known dangerous protocols outright.
    for danger in [
        "ext::",
        "file://",
        "ftp://",
        "ftps://",
        "gopher://",
        "rsync://",
    ] {
        if lower.starts_with(danger) {
            return Err(bad("disallowed scheme"));
        }
    }

    // Allow well-formed schemes.
    let allowed_scheme = [
        "https://",
        "http://",
        "ssh://",
        "git://",
        "git+https://",
        "git+ssh://",
    ]
    .iter()
    .any(|p| lower.starts_with(p));

    // Allow scp-like syntax: user@host:path (no scheme, must contain `@` and `:`,
    // and the colon must precede any `/`).
    let scp_like = !trimmed.contains("://")
        && trimmed.contains('@')
        && trimmed
            .find(':')
            .is_some_and(|c| trimmed[..c].contains('@') && !trimmed[..c].is_empty());

    if !(allowed_scheme || scp_like) {
        return Err(bad(
            "scheme must be https/http/ssh/git or user@host:path form",
        ));
    }

    Ok(())
}

// ─── Parsing Helpers ─────────────────────────────────────────

/// Parse "git version X.Y.Z" into GitVersion.
pub fn parse_git_version(output: &str) -> Result<GitVersion, SyncError> {
    // Expected format: "git version 2.39.1" or "git version 2.39.1 (Apple Git-145)"
    let version_part = output
        .trim()
        .strip_prefix("git version ")
        .unwrap_or(output.trim());

    let parts: Vec<&str> = version_part.split('.').collect();
    if parts.len() < 2 {
        return Err(SyncError::GitCommandFailed {
            command: "git --version".to_string(),
            stderr: format!("unexpected version format: {}", output),
        });
    }

    let major = parts[0]
        .parse::<u32>()
        .map_err(|_| SyncError::GitCommandFailed {
            command: "git --version".to_string(),
            stderr: format!("cannot parse major version: {}", parts[0]),
        })?;

    let minor = parts[1]
        .parse::<u32>()
        .map_err(|_| SyncError::GitCommandFailed {
            command: "git --version".to_string(),
            stderr: format!("cannot parse minor version: {}", parts[1]),
        })?;

    let patch = if parts.len() >= 3 {
        // Handle "2.39.1 (Apple Git-145)" — take only digits
        let patch_str: String = parts[2]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        patch_str.parse::<u32>().unwrap_or(0)
    } else {
        0
    };

    Ok(GitVersion {
        major,
        minor,
        patch,
    })
}

/// Parse `git status --porcelain` output.
pub fn parse_git_status(output: &str) -> Vec<FileStatus> {
    output
        .lines()
        .filter(|line| line.len() >= 3)
        .filter_map(|line| {
            let status_code = &line[..2];
            let path = line[3..].to_string();

            let kind = match status_code.trim() {
                "A" | "AM" => Some(FileStatusKind::Added),
                "M" | "MM" => Some(FileStatusKind::Modified),
                "D" => Some(FileStatusKind::Deleted),
                "??" => Some(FileStatusKind::Untracked),
                _ => Some(FileStatusKind::Modified), // Catch-all for other changes
            };

            kind.map(|status| FileStatus { path, status })
        })
        .collect()
}

/// Parse `git log --format=...` output.
pub fn parse_git_log(output: &str) -> Vec<GitCommitInfo> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(5, '|').collect();
            if parts.len() == 5 {
                Some(GitCommitInfo {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    date: parts[2].to_string(),
                    message: parts[3].to_string(),
                    author: parts[4].to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Parse conflicted file paths from git merge output.
fn parse_conflict_files(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter(|line| line.contains("CONFLICT") || line.contains("Merge conflict in"))
        .filter_map(|line| {
            // "CONFLICT (content): Merge conflict in filename.toml"
            line.rsplit("in ").next().map(|s| s.trim().to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_version_standard() {
        let v = parse_git_version("git version 2.39.1").unwrap();
        assert_eq!(
            v,
            GitVersion {
                major: 2,
                minor: 39,
                patch: 1
            }
        );
    }

    #[test]
    fn test_parse_git_version_apple() {
        let v = parse_git_version("git version 2.39.3 (Apple Git-146)").unwrap();
        assert_eq!(
            v,
            GitVersion {
                major: 2,
                minor: 39,
                patch: 3
            }
        );
    }

    #[test]
    fn test_parse_git_version_two_parts() {
        let v = parse_git_version("git version 2.28").unwrap();
        assert_eq!(
            v,
            GitVersion {
                major: 2,
                minor: 28,
                patch: 0
            }
        );
    }

    #[test]
    fn test_git_version_meets_minimum() {
        assert!(GitVersion {
            major: 2,
            minor: 39,
            patch: 0
        }
        .meets_minimum());
        assert!(GitVersion {
            major: 2,
            minor: 28,
            patch: 0
        }
        .meets_minimum());
        assert!(!GitVersion {
            major: 2,
            minor: 27,
            patch: 9
        }
        .meets_minimum());
        assert!(!GitVersion {
            major: 1,
            minor: 99,
            patch: 0
        }
        .meets_minimum());
        assert!(GitVersion {
            major: 3,
            minor: 0,
            patch: 0
        }
        .meets_minimum());
    }

    #[test]
    fn test_parse_git_status() {
        let output = " M file1.rs\n?? file2.rs\nA  file3.rs\n D file4.rs\n";
        let statuses = parse_git_status(output);
        assert_eq!(statuses.len(), 4);
        assert_eq!(statuses[0].path, "file1.rs");
        assert_eq!(statuses[0].status, FileStatusKind::Modified);
        assert_eq!(statuses[1].path, "file2.rs");
        assert_eq!(statuses[1].status, FileStatusKind::Untracked);
        assert_eq!(statuses[2].path, "file3.rs");
        assert_eq!(statuses[2].status, FileStatusKind::Added);
        assert_eq!(statuses[3].path, "file4.rs");
        assert_eq!(statuses[3].status, FileStatusKind::Deleted);
    }

    #[test]
    fn test_parse_git_log() {
        let output = "abc1234567890|abc1234|2026-04-15T10:00:00+03:00|initial commit|Emre\ndef5678901234|def5678|2026-04-15T11:00:00+03:00|update config|Emre\n";
        let commits = parse_git_log(output);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].short_hash, "abc1234");
        assert_eq!(commits[0].message, "initial commit");
        assert_eq!(commits[1].message, "update config");
    }

    #[test]
    fn test_parse_git_status_empty() {
        let statuses = parse_git_status("");
        assert!(statuses.is_empty());
    }

    #[test]
    fn test_parse_git_log_empty() {
        let commits = parse_git_log("");
        assert!(commits.is_empty());
    }

    #[test]
    fn test_validate_remote_url_accepts_https_ssh_scp() {
        assert!(validate_remote_url("https://github.com/u/r.git").is_ok());
        assert!(validate_remote_url("ssh://git@github.com/u/r.git").is_ok());
        assert!(validate_remote_url("git@github.com:u/r.git").is_ok());
        assert!(validate_remote_url("git://github.com/u/r.git").is_ok());
    }

    #[test]
    fn test_validate_remote_url_rejects_dangerous() {
        assert!(validate_remote_url("ext::sh -c 'rm -rf /'").is_err());
        assert!(validate_remote_url("file:///etc/passwd").is_err());
        assert!(validate_remote_url("-upload-pack=evil").is_err());
        assert!(validate_remote_url("").is_err());
        assert!(validate_remote_url("https://x.com/r\n--upload-pack=x").is_err());
        assert!(validate_remote_url("/local/path").is_err());
        assert!(validate_remote_url("rsync://x").is_err());
    }
}
