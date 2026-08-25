//! PID file: exclusive daemon startup via an advisory file lock.
//!
//! Holding the lock *is* the definition of "daemon alive" — it is released
//! automatically on exit, including SIGKILL. The file's contents are only a
//! hint for `stop`; readers must always verify liveness separately. The file
//! is deliberately never removed: a stale leftover is harmless (the next
//! `acquire` overwrites it) and unlinking would race a concurrent acquirer
//! into deleting a live PID file.

use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process,
};

/// Why [`PidFile::acquire`] failed.
#[derive(Debug, thiserror::Error)]
pub enum AcquireError {
    /// Another daemon holds the lock. `pid` is best-effort from the file's
    /// contents and may be `None` if it is absent or unreadable.
    #[error("daemon already running{}", pid.as_ref().map_or_else(String::new, |p| format!(" (pid {p})")))]
    AlreadyRunning { pid: Option<u32> },
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// An exclusively-held PID file. Drop to release the lock.
pub struct PidFile {
    path: PathBuf,
    file: File,
}

impl PidFile {
    /// Create and lock `path`, recording this process's PID. Fails with
    /// [`AcquireError::AlreadyRunning`] if another live daemon holds it.
    pub fn acquire(path: impl Into<PathBuf>) -> Result<Self, AcquireError> {
        let path = path.into();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(AcquireError::AlreadyRunning {
                    pid: read_pid(&path)?,
                });
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }

        let mut pid_file = Self { path, file };
        pid_file.write_pid()?;
        Ok(pid_file)
    }

    /// This process's PID as recorded in the file.
    pub fn pid(&self) -> u32 {
        process::id()
    }

    /// The path of the underlying PID file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn write_pid(&mut self) -> io::Result<()> {
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        writeln!(self.file, "{}", self.pid())?;
        self.file.sync_all()?;
        Ok(())
    }
}

impl Drop for PidFile {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// Best-effort read of the PID recorded at `path`: `None` when the file is
/// missing, empty or unparseable.
pub fn read_pid(path: &Path) -> io::Result<Option<u32>> {
    let mut buf = String::new();
    File::open(path)?.read_to_string(&mut buf)?;
    Ok(buf.trim().parse().ok())
}
