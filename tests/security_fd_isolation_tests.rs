/// @threat T-001 @layer L1 @control C-001
/// Verify that after sandbox_command (close_fds_before_exec), child processes
/// see only the standard file descriptors (stdin, stdout, stderr).
use std::os::fd::AsRawFd;
use std::process::Stdio;

/// L1 — Unit: Child process enumerates its FDs and confirms no inherited FDs.
///
/// Opens a temp file in the parent, calls sandbox_command() on a child process,
/// and checks the child's fd list. The child should see at most ~10 fds
/// (stdin/stdout/stderr + shell/ls pipe fds), NOT hundreds including the
/// parent's temp file fd.
#[test]
fn test_fd_isolation_child_enumerates_no_inherited_fds() {
    // Open a temp file in the parent — this fd should NOT be inherited
    let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");

    // Shell script that counts visible file descriptors
    let fd_count_script = r#"
        if [ -d /dev/fd ]; then
            ls -1 /dev/fd 2>/dev/null | wc -l
        elif [ -d /proc/self/fd ]; then
            ls -1 /proc/self/fd 2>/dev/null | wc -l
        else
            echo 999
        fi
    "#;

    let mut cmd = std::process::Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(fd_count_script)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    envforge::ops::secrets::provider::sandbox_command(&mut cmd);

    let output = cmd.output().expect("failed to spawn child");
    let fd_count: usize = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(999);

    assert!(
        fd_count <= 10,
        "Child inherited {} FDs after sandbox_command. Expected <=10 (stdin/stdout/stderr + shell/ls pipe fds). Parent had extra temp file FD open.",
        fd_count
    );

    // Keep temp alive to ensure the test is meaningful
    let _ = temp;
}

/// L1 — Unit: Verify CloexecFile sets FD_CLOEXEC at construction time.
#[test]
fn test_cloexec_file_has_cloexec_flag() {
    let temp = tempfile::NamedTempFile::new().unwrap();
    let cloexec = envforge::ops::secrets::CloexecFile::from(temp.as_file().try_clone().unwrap());
    let fd = cloexec.as_raw_fd();

    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFD);
        assert!(flags != -1, "fcntl(F_GETFD) failed");
        assert!(
            flags & libc::FD_CLOEXEC != 0,
            "FD_CLOEXEC not set on CloexecFile fd"
        );
    }
}

/// L2 — Integration: Verify sandbox_command is applied by multiple spawn paths.
///
/// Verifies FD isolation works through different Command configuration patterns
/// (piped stdin, env vars) to catch regression in any path.
#[test]
fn test_fd_isolation_via_sandbox_command() {
    let temp = tempfile::NamedTempFile::new().expect("failed to create temp file");

    let fd_count_script = r#"
        if [ -d /dev/fd ]; then
            ls -1 /dev/fd 2>/dev/null | wc -l
        elif [ -d /proc/self/fd ]; then
            ls -1 /proc/self/fd 2>/dev/null | wc -l
        else
            echo 999
        fi
    "#;

    // Test with piped stdin (simulates run_cli_with_stdin path)
    let mut cmd = std::process::Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(fd_count_script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    envforge::ops::secrets::provider::sandbox_command(&mut cmd);

    let output = cmd.output().expect("failed to spawn child");
    let fd_count: usize = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(999);

    assert!(
        fd_count <= 10,
        "Child with piped stdin inherited {} FDs after sandbox_command. Expected <=10.",
        fd_count
    );

    let _ = temp;
}

/// L3 — Attack Simulation: Child attempts to read inherited FDs and exfiltrate content.
///
/// Opens a temp file with a known secret string, spawns a "malicious" child
/// that reads /proc/self/fd/* (or /dev/fd/*), and asserts the secret does NOT
/// appear in the child's output.
#[test]
fn test_fd_isolation_malicious_child_cannot_read_inherited_secrets() {
    let secret = "ENVFGD_SECRET_TEST_MARKER_a3f2b11c";
    let mut temp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut temp, secret.as_bytes()).unwrap();
    std::io::Write::flush(&mut temp).unwrap();

    let exfil_script = r#"
        for d in /dev/fd /proc/self/fd; do
            if [ -d "$d" ]; then
                for fd in "$d"/*; do
                    cat "$fd" 2>/dev/null || true
                done
            fi
        done
    "#;

    let mut cmd = std::process::Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(exfil_script)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    envforge::ops::secrets::provider::sandbox_command(&mut cmd);

    let output = cmd.output().expect("failed to spawn child");
    let child_stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !child_stdout.contains(secret),
        "MALICIOUS CHILD READ INHERITED SECRET! Child stdout contained '{}'. FD isolation failed.",
        secret
    );

    let _ = temp;
}
