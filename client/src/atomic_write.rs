//! Files of the game folder that must never be left half written.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Writes `data` to `path` through `<path>.tmp`, synced then renamed over it, so that a failed
/// write or a crash leaves the previous file. With `private`, the file is readable by its owner
/// only on Unix (mode 0600), from the moment it exists.
pub fn write_atomically(path: &Path, data: &[u8], private: bool) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    // A leftover of a crash keeps the mode it was created with
    let _ = fs::remove_file(&tmp);
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut file = options.open(&tmp)?;
    file.write_all(data)?;
    file.sync_all()?;
    fs::rename(&tmp, path)
}
