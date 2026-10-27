//! Persistent file-manager places and per-folder column layouts.
//!
//! Folder overrides take precedence over the global layout in
//! `Preferences::files.columns`. Applying a layout only to the current folder
//! means calling [`Places::set_columns`] and saving. Applying it globally means
//! saving it as the global preference; the interface then clears the current
//! folder override with [`Places::clear_columns`] so that folder inherits the
//! new global layout. Other folder overrides are never changed automatically.

use std::{
    ffi::OsString,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use crate::settings::columns::Layout;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const RECENT_LIMIT: usize = 100;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Places {
    // OsString's Serde representation retains non-UTF-8 Unix paths.
    pub favorites: Vec<OsString>,
    pub recent: Vec<OsString>,
    pub pinned: Vec<OsString>,
    /// Exact-path column overrides; prefer the `columns_*` methods.
    pub column_overrides: Vec<FolderColumns>,
}

/// A column layout override for exactly one folder.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FolderColumns {
    /// Raw OS path; JSON map keys cannot represent non-UTF-8 Unix paths losslessly.
    pub path: OsString,
    #[serde(default)]
    pub columns: Layout,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("the user state directory is not available")]
    NoStateDirectory,
    #[error("cannot read or write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid places data: {0}")]
    Data(#[from] serde_json::Error),
    #[error("refusing to replace a symbolic link or read-only state file: {0}")]
    ReadOnly(PathBuf),
}

impl Places {
    /// Load saved places, or an empty collection on the first launch.
    ///
    /// # Errors
    /// Reports an unavailable state directory, malformed data or a failed read.
    pub fn load() -> Result<Self, Error> {
        load_from(&state_path()?)
    }

    pub fn record(&mut self, path: &Path) {
        self.recent.retain(|old| old != path.as_os_str());
        self.recent.insert(0, path.as_os_str().to_owned());
        self.recent.truncate(RECENT_LIMIT);
    }

    pub fn is_favorite(&self, path: &Path) -> bool {
        self.favorites.iter().any(|entry| entry == path.as_os_str())
    }

    pub fn toggle_favorite(&mut self, path: &Path) {
        if self.is_favorite(path) {
            self.favorites.retain(|entry| entry != path.as_os_str());
        } else {
            self.favorites.push(path.as_os_str().to_owned());
        }
    }

    /// The column override recorded for exactly `path`.
    ///
    /// Parent and child folders do not inherit each other's overrides.
    pub fn columns_for(&self, path: &Path) -> Option<&Layout> {
        self.column_overrides
            .iter()
            .find(|entry| entry.path.as_os_str() == path.as_os_str())
            .map(|entry| &entry.columns)
    }

    /// Record the column layout for exactly `path`, replacing an existing override.
    ///
    /// This does not modify the global `Preferences::files.columns` or any other
    /// folder.
    pub fn set_columns(&mut self, path: &Path, columns: Layout) {
        let path = path.as_os_str();
        self.column_overrides
            .retain(|entry| entry.path.as_os_str() != path);
        self.column_overrides.push(FolderColumns {
            path: path.to_owned(),
            columns,
        });
    }

    /// Remove the override for exactly `path` so it uses the global layout again.
    /// Returns whether an override was removed.
    pub fn clear_columns(&mut self, path: &Path) -> bool {
        let before = self.column_overrides.len();
        self.column_overrides
            .retain(|entry| entry.path.as_os_str() != path.as_os_str());
        before != self.column_overrides.len()
    }

    /// The layout that applies at `path`: its override, otherwise `global`.
    pub fn effective_columns<'a>(&'a self, path: &Path, global: &'a Layout) -> &'a Layout {
        self.columns_for(path).unwrap_or(global)
    }

    /// Atomically save application-owned place records.
    ///
    /// # Errors
    /// Reports serialization or I/O failures and refuses to replace read-only links.
    pub fn save(&self) -> Result<(), Error> {
        save_to(&state_path()?, self)
    }
}

/// # Errors
/// Reports malformed data or a read failure; a missing file is an empty collection.
fn load_from(path: &Path) -> Result<Places, Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Places::default()),
        Err(source) => Err(Error::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

/// # Errors
/// Reports serialization or I/O failures and refuses to replace read-only links.
fn save_to(path: &Path, places: &Places) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.permissions().readonly() => {
            return Err(Error::ReadOnly(path.to_owned()));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(Error::Io {
                path: path.to_owned(),
                source,
            });
        }
    }
    let parent = path
        .parent()
        .expect("the state file has a parent directory");
    fs::create_dir_all(parent).map_err(|source| Error::Io {
        path: parent.to_owned(),
        source,
    })?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })?;
    temporary
        .write_all(&serde_json::to_vec_pretty(places)?)
        .map_err(|source| Error::Io {
            path: path.to_owned(),
            source,
        })?;
    temporary.persist(path).map_err(|error| Error::Io {
        path: path.to_owned(),
        source: error.error,
    })?;
    Ok(())
}

/// # Errors
/// Returns `NoStateDirectory` when the platform cannot determine a user state path.
fn state_path() -> Result<PathBuf, Error> {
    let dirs = BaseDirs::new().ok_or(Error::NoStateDirectory)?;
    Ok(dirs
        .state_dir()
        .unwrap_or(dirs.data_local_dir())
        .join("gds/files/places.json"))
}

#[cfg(test)]
mod tests;
