//! Sync subprocess executor trait with timeout-bounded execution.
//!
//! Manual polling watchdog avoids tokio runtime requirement.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::ResolverError;

const POLL_INTERVAL_MS: u64 = 50;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SubprocessOutcome {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub elapsed: Duration,
}

pub trait SubprocessExecutor: Send + Sync {
    fn execute(
        &self,
        cmd: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<SubprocessOutcome, ResolverError>;
}

pub struct StdSubprocessExecutor;

impl SubprocessExecutor for StdSubprocessExecutor {
    fn execute(
        &self,
        cmd: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<SubprocessOutcome, ResolverError> {
        let started = Instant::now();
        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ResolverError::Io {
                context: format!("spawn '{cmd}'"),
                source: e,
            })?;

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let exit_code = status.code().unwrap_or(-1);
                    let mut stdout = Vec::new();
                    let mut stderr = Vec::new();
                    if let Some(s) = child.stdout.take() {
                        let _ = s.take(MAX_OUTPUT_BYTES as u64).read_to_end(&mut stdout);
                    }
                    if let Some(s) = child.stderr.take() {
                        let _ = s.take(MAX_OUTPUT_BYTES as u64).read_to_end(&mut stderr);
                    }
                    return Ok(SubprocessOutcome {
                        stdout,
                        stderr,
                        exit_code,
                        elapsed: started.elapsed(),
                    });
                }
                Ok(None) => {
                    if started.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(ResolverError::SubprocessTimeout {
                            cmd: cmd.to_string(),
                            elapsed_ms: started.elapsed().as_millis(),
                        });
                    }
                    std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                }
                Err(e) => {
                    return Err(ResolverError::Io {
                        context: format!("wait '{cmd}'"),
                        source: e,
                    });
                }
            }
        }
    }
}

/// Cap stderr excerpt for use in `SubprocessFailed` errors. Hard limit at
/// 256 bytes to avoid leaking long shell output into audit logs.
pub fn truncate_stderr(stderr: &[u8]) -> String {
    let max = 256.min(stderr.len());
    String::from_utf8_lossy(&stderr[..max]).into_owned()
}
