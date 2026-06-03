use std::fs::File;
use std::io;
use std::ops::{Deref, DerefMut};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

/// A `File` wrapper that guarantees FD_CLOEXEC is set.
///
/// On construction, the file descriptor is marked close-on-exec via `fcntl(F_SETFD, FD_CLOEXEC)`.
/// This prevents child processes spawned by EnvForge from inheriting open file descriptors,
/// closing the FD leak attack vector (see `.threatmodel/T-001.yaml`).
///
/// # Safety Invariant
///
/// The inner `File` always has `FD_CLOEXEC` set. Any code that converts a raw `File` into a
/// `CloexecFile` via `From<File>` sets the flag before wrapping. Any code that constructs
/// a `CloexecFile` from a raw fd sets the flag before wrapping.
///
/// # Examples
///
/// ```no_run
/// use envforge::ops::secrets::CloexecFile;
///
/// let file = CloexecFile::open("/tmp/secret")?;
/// // File descriptor is CLOEXEC. Child processes won't inherit it.
/// # Ok::<_, std::io::Error>(())
/// ```
#[derive(Debug)]
pub struct CloexecFile(File);

impl CloexecFile {
    /// Open a file at `path` with `FD_CLOEXEC` set.
    ///
    /// Uses `O_CLOEXEC` at open time where available (Linux 2.6.23+, macOS 10.7+,
    /// FreeBSD 8.3+). Falls back to `fcntl(F_SETFD, FD_CLOEXEC)` after open on
    /// platforms without `O_CLOEXEC`.
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> io::Result<Self> {
        let file = Self::open_cloexec(path)?;
        Ok(CloexecFile(file))
    }

    /// Create a new file with `FD_CLOEXEC` set.
    pub fn create<P: AsRef<std::path::Path>>(path: P) -> io::Result<Self> {
        let file = File::create(path)?;
        set_cloexec(file.as_raw_fd())?;
        Ok(CloexecFile(file))
    }

    fn open_cloexec<P: AsRef<std::path::Path>>(path: P) -> io::Result<File> {
        use std::os::unix::fs::OpenOptionsExt;

        let mut opts = std::fs::OpenOptions::new();
        opts.read(true).write(true).create(true);

        #[cfg(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            opts.custom_flags(libc::O_CLOEXEC);
        }

        let file = opts.open(path)?;

        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        {
            set_cloexec(file.as_raw_fd())?;
        }

        Ok(file)
    }
}

impl Deref for CloexecFile {
    type Target = File;

    fn deref(&self) -> &File {
        &self.0
    }
}

impl DerefMut for CloexecFile {
    fn deref_mut(&mut self) -> &mut File {
        &mut self.0
    }
}

impl From<File> for CloexecFile {
    /// Convert a raw `File` into a `CloexecFile`, setting FD_CLOEXEC on the handle.
    ///
    /// # Panics
    ///
    /// Panics if `fcntl(F_SETFD, FD_CLOEXEC)` fails. This should only happen on
    /// a closed or invalid fd, which indicates a programming error.
    fn from(file: File) -> Self {
        set_cloexec(file.as_raw_fd()).expect("fcntl(F_SETFD, FD_CLOEXEC) failed on live fd");
        CloexecFile(file)
    }
}

impl IntoRawFd for CloexecFile {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

impl AsRawFd for CloexecFile {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl FromRawFd for CloexecFile {
    /// # Safety
    ///
    /// The caller must ensure `fd` is a valid, open file descriptor.
    /// `FD_CLOEXEC` is set on the fd before wrapping.
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        set_cloexec(fd).expect("fcntl(F_SETFD, FD_CLOEXEC) failed on raw fd");
        CloexecFile(File::from_raw_fd(fd))
    }
}

/// Set the close-on-exec flag on a file descriptor.
///
/// Uses `fcntl(fd, F_SETFD, FD_CLOEXEC)`. This is the fallback for platforms
/// that don't support `O_CLOEXEC` in `open()`, and is also called by
/// `From<File>` and `FromRawFd` to retrofit existing FDs.
fn set_cloexec(fd: RawFd) -> io::Result<()> {
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFD);
        if flags == -1 {
            return Err(io::Error::last_os_error());
        }
        if libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloexec_file_has_cloexec_flag() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let cloexec = CloexecFile::from(temp.as_file().try_clone().unwrap());
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

    #[test]
    fn test_cloexec_file_open_sets_flag() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        let file = CloexecFile::open(&path).unwrap();
        let fd = file.as_raw_fd();

        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            assert!(flags != -1, "fcntl(F_GETFD) failed");
            assert!(
                flags & libc::FD_CLOEXEC != 0,
                "FD_CLOEXEC not set on CloexecFile fd after open()"
            );
        }
    }
}
