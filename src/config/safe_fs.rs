use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub fn safe_read_to_string(path: &Path) -> std::io::Result<String> {
    let mut f = open_read_no_follow(path)?;
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;
    Ok(buf)
}

pub fn safe_write(path: &Path, content: &str) -> std::io::Result<()> {
    let mut f = open_write_no_follow(path)?;
    f.write_all(content.as_bytes())?;
    f.sync_all()?;
    Ok(())
}

fn open_read_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    opts.open(path)
}

fn open_write_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    opts.open(path)
}
