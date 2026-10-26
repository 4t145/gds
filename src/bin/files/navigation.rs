use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::directory::{self, DirectorySnapshot, ReadOptions};

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ViewState {
    pub selected: Vec<PathBuf>,
    /// Vertical offset in the UI's scroll-coordinate convention.
    pub scroll_offset: f32,
}

#[derive(Debug, Clone)]
pub struct Location {
    pub path: PathBuf,
    pub view: ViewState,
}

#[derive(Debug)]
pub struct Tab {
    pub pinned: bool,
    pub navigation: Navigation,
}

impl Tab {
    /// Construct a tab from a successfully loaded directory, without performing I/O.
    pub fn new(directory: DirectorySnapshot) -> Self {
        Self {
            pinned: false,
            navigation: Navigation::new(directory),
        }
    }

    pub fn title(&self) -> String {
        let path = self.navigation.path();
        path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

#[derive(Debug)]
pub struct Navigation {
    directory: DirectorySnapshot,
    history: Vec<Location>,
    history_index: usize,
    pending: Option<LoadRequest>,
}

#[derive(Debug, Clone, Copy)]
enum Destination {
    Open,
    History { index: usize },
    Refresh,
}

/// Owned, cloneable request suitable for moving to a background executor.
#[derive(Debug, Clone)]
pub struct LoadRequest {
    id: u64,
    path: PathBuf,
    options: ReadOptions,
    destination: Destination,
}

impl LoadRequest {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn options(&self) -> ReadOptions {
        self.options
    }

    /// Perform the blocking filesystem read, independently of the navigation model.
    ///
    /// # Errors
    /// Returns the directory-read error when the target cannot be opened.
    pub fn load(&self) -> Result<DirectorySnapshot, directory::Error> {
        directory::read_directory(&self.path, self.options)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadOutcome {
    Applied,
    /// This request was cancelled, superseded, already applied, or belongs to another tab.
    Stale,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("a location is required")]
    EmptyInput,
    #[error("URI locations are not supported in the local-only version: {input}")]
    UnsupportedUri { input: String },
    #[error("only ~ and ~/ home expansion are supported: {input}")]
    UnsupportedHomeExpansion { input: String },
    #[error("the home directory is not available")]
    MissingHome,
    #[error("the base directory must be absolute: {path}")]
    RelativeBase { path: PathBuf },
    #[error(transparent)]
    Load(#[from] directory::Error),
    #[error("loaded directory {actual} does not match requested directory {expected}")]
    UnexpectedSnapshot { expected: PathBuf, actual: PathBuf },
}

/// Resolve typed local input without filesystem access or shell expansion.
///
/// Relative paths use the tab's current directory, not the process working
/// directory. Whitespace and `..` are preserved: lexical removal of `..` would
/// change the meaning of paths traversing a symlink. Non-UTF-8 paths obtained
/// from directory entries can instead be passed directly to `prepare_open`.
///
/// # Errors
/// Rejects empty input, URI scheme prefixes, unsupported ~user expansion,
/// missing home directories, or a relative base/home directory. A literal
/// relative filename containing a scheme-like colon can be entered with `./`.
pub fn resolve_input(
    input: &str,
    current_directory: &Path,
    home: Option<&Path>,
) -> Result<PathBuf, Error> {
    if input.is_empty() {
        return Err(Error::EmptyInput);
    }
    if Path::new(input).is_absolute() {
        return Ok(PathBuf::from(input));
    }
    if is_uri(input) {
        return Err(Error::UnsupportedUri {
            input: input.to_owned(),
        });
    }
    if input == "~" || input.starts_with("~/") {
        let home = home.ok_or(Error::MissingHome)?;
        require_absolute(home)?;
        return Ok(match input.strip_prefix("~/") {
            Some(relative) => home.join(relative.trim_start_matches('/')),
            None => home.to_path_buf(),
        });
    }
    if input.starts_with('~') {
        return Err(Error::UnsupportedHomeExpansion {
            input: input.to_owned(),
        });
    }
    require_absolute(current_directory)?;
    Ok(current_directory.join(input))
}

fn is_uri(input: &str) -> bool {
    let Some((scheme, _)) = input.split_once(':') else {
        return false;
    };
    let mut characters = scheme.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

/// # Errors
/// Returns `RelativeBase` for a non-absolute directory.
fn require_absolute(path: &Path) -> Result<(), Error> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(Error::RelativeBase {
            path: path.to_path_buf(),
        })
    }
}

impl Navigation {
    /// Construct from a successful `read_directory` result. This performs no I/O.
    pub fn new(directory: DirectorySnapshot) -> Self {
        let initial = Location {
            path: directory.path.clone(),
            view: ViewState::default(),
        };
        Self {
            directory,
            history: vec![initial],
            history_index: 0,
            pending: None,
        }
    }

    pub fn directory(&self) -> &DirectorySnapshot {
        &self.directory
    }

    pub fn path(&self) -> &Path {
        &self.directory.path
    }

    pub fn history(&self) -> &[Location] {
        &self.history
    }

    pub fn history_index(&self) -> usize {
        self.history_index
    }

    pub fn view(&self) -> &ViewState {
        &self.history[self.history_index].view
    }

    pub fn view_mut(&mut self) -> &mut ViewState {
        &mut self.history[self.history_index].view
    }

    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }

    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }

    /// Prepare a local navigation without changing the visible location or doing I/O.
    /// Relative paths are relative to this tab. For typed text, use `prepare_input`.
    pub fn prepare_open(&mut self, path: impl AsRef<Path>) -> LoadRequest {
        let path = path.as_ref();
        let target = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.path().join(path)
        };
        self.begin(target, self.directory.options, Destination::Open)
    }

    /// Parse user input and prepare a background load, without filesystem I/O.
    ///
    /// # Errors
    /// Returns the input error from `resolve_input`; the visible state is unchanged.
    pub fn prepare_input(
        &mut self,
        input: &str,
        home: Option<&Path>,
    ) -> Result<LoadRequest, Error> {
        let path = resolve_input(input, self.path(), home)?;
        Ok(self.prepare_open(path))
    }

    pub fn prepare_back(&mut self) -> Option<LoadRequest> {
        let index = self.history_index.checked_sub(1)?;
        Some(self.prepare_history(index))
    }

    pub fn prepare_forward(&mut self) -> Option<LoadRequest> {
        if !self.can_go_forward() {
            return None;
        }
        Some(self.prepare_history(self.history_index + 1))
    }

    pub fn prepare_parent(&mut self) -> Option<LoadRequest> {
        let parent = self.path().parent()?.to_path_buf();
        Some(self.prepare_open(parent))
    }

    /// Refresh or change the hidden-file filter only after the load succeeds.
    pub fn prepare_refresh(&mut self, options: ReadOptions) -> LoadRequest {
        self.begin(self.path().to_path_buf(), options, Destination::Refresh)
    }

    pub fn cancel_pending(&mut self) {
        self.pending = None;
    }

    /// Commit a completed background read to the originating tab.
    ///
    /// Stale completions, including their errors, are explicitly ignored. The
    /// currently displayed directory, history and view state survive a failed
    /// current load. Back/forward restore the visited location's view state.
    ///
    /// # Errors
    /// Returns `Load` for a failed current read or `UnexpectedSnapshot` if the
    /// supplied result does not match the request. Neither commits navigation.
    pub fn finish_load(
        &mut self,
        request: &LoadRequest,
        result: Result<DirectorySnapshot, directory::Error>,
    ) -> Result<LoadOutcome, Error> {
        if !self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.id == request.id)
        {
            return Ok(LoadOutcome::Stale);
        }
        self.pending = None;
        let directory = result?;
        if directory.path != request.path {
            return Err(Error::UnexpectedSnapshot {
                expected: request.path.clone(),
                actual: directory.path,
            });
        }
        match request.destination {
            Destination::Open if directory.path != self.directory.path => {
                self.history.truncate(self.history_index + 1);
                self.history.push(Location {
                    path: directory.path.clone(),
                    view: ViewState::default(),
                });
                self.history_index = self.history.len() - 1;
            }
            Destination::History { index } => self.history_index = index,
            Destination::Open | Destination::Refresh => {}
        }
        self.directory = directory;
        let entries = &self.directory.entries;
        self.history[self.history_index]
            .view
            .selected
            .retain(|path| entries.iter().any(|entry| entry.path == *path));
        Ok(LoadOutcome::Applied)
    }

    fn prepare_history(&mut self, index: usize) -> LoadRequest {
        self.begin(
            self.history[index].path.clone(),
            self.directory.options,
            Destination::History { index },
        )
    }

    fn begin(
        &mut self,
        path: PathBuf,
        options: ReadOptions,
        destination: Destination,
    ) -> LoadRequest {
        let request = LoadRequest {
            id: NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed),
            path,
            options,
            destination,
        };
        self.pending = Some(request.clone());
        request
    }
}

#[cfg(test)]
mod tests;
