//! Where Pulsline keeps its data.
//!
//! Everything lives under one data directory: `~/.local/share/pulsline` on
//! Linux (the platform's data directory elsewhere), or `$PULSLINE_DATA_DIR`
//! when it is set — which is how tests and a second, throwaway instance keep
//! clear of the real one.

use std::path::PathBuf;

use anyhow::Context as _;

/// The environment variable that moves the data directory.
pub const DATA_DIR_ENV: &str = "PULSLINE_DATA_DIR";

/// Pulsline's paths on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub data_dir: PathBuf,
}

impl Paths {
    /// The paths for this user: `$PULSLINE_DATA_DIR`, or the platform's data
    /// directory.
    pub fn resolve() -> anyhow::Result<Self> {
        if let Some(dir) = std::env::var_os(DATA_DIR_ENV).filter(|d| !d.is_empty()) {
            return Ok(Self {
                data_dir: PathBuf::from(dir),
            });
        }
        let dirs = directories::ProjectDirs::from("", "", "pulsline")
            .context("no home directory to keep Pulsline's data in")?;
        Ok(Self {
            data_dir: dirs.data_dir().to_path_buf(),
        })
    }

    /// The spool: append-only JSONL, the source of truth.
    pub fn spool_dir(&self) -> PathBuf {
        self.data_dir.join("spool")
    }

    /// The SQLite index built from the spool.
    pub fn index_path(&self) -> PathBuf {
        self.data_dir.join("index.db")
    }
}
