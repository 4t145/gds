//! Single-file selection mode for `gds-files`.
//!
//! The picker turns the browser into a modal file chooser: the user picks one
//! file, and the chosen path is reported as one JSON line on stdout. Filter
//! groups narrow the file list without blocking directory navigation, and the
//! matcher works on the raw basename so non-UTF-8 names still match.

use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use thiserror::Error;

/// Exit status reported after the window closes: the user confirmed a file.
const EXIT_CONFIRMED: u8 = 0;
/// Exit status for cancellation, window close and startup failures.
const EXIT_CANCELLED: u8 = 1;
/// Exit status when the selection could not be written to stdout.
const EXIT_FAILED: u8 = 2;

/// Outcome of one picker session, shared with the process exit path.
static EXIT: AtomicU8 = AtomicU8::new(EXIT_CANCELLED);

/// # Errors
/// Reports unusable command-line arguments.
pub fn parse<I>(arguments: I) -> Result<Arguments, Error>
where
    I: IntoIterator<Item = OsString>,
{
    let mut arguments = arguments.into_iter();
    let mut directory = None;
    let mut requested = Requested::default();
    let mut positional_only = false;

    while let Some(argument) = arguments.next() {
        if positional_only {
            directory = Some(directory_argument(directory, argument)?);
            continue;
        }
        match argument.to_str() {
            Some("--") => positional_only = true,
            Some("-h" | "--help") => requested.help = true,
            Some("-V" | "--version") => requested.version = true,
            Some("--pick-file") => requested.picker = true,
            Some("--read-only") => requested.read_only = Some("--read-only"),
            Some(value) if value.starts_with("--filter=") => {
                requested
                    .filters
                    .push(Filter::from_spec(&value["--filter=".len()..])?);
            }
            Some("--filter") => {
                let Some(value) = arguments.next() else {
                    return Err(Error::MissingValue { option: "--filter" });
                };
                let Some(value) = value.to_str() else {
                    return Err(Error::NonUtf8Filter);
                };
                requested.filters.push(Filter::from_spec(value)?);
            }
            Some(value) if value.starts_with('-') && value != "-" => {
                return Err(Error::UnknownArgument { argument });
            }
            _ => directory = Some(directory_argument(directory, argument)?),
        }
    }

    if !requested.picker {
        if let Some(option) = requested.read_only {
            return Err(Error::PickerOnlyOption { option });
        }
        if !requested.filters.is_empty() {
            return Err(Error::PickerOnlyOption { option: "--filter" });
        }
    }
    let picker = requested.picker.then(|| Options {
        filters: normalized_filters(requested.filters),
        read_only: requested.read_only.is_some(),
    });
    Ok(Arguments {
        directory,
        picker,
        help: requested.help,
        version: requested.version,
    })
}

/// Placement of one positional directory argument.
///
/// A second directory is an error: the browser has a single start location.
fn directory_argument(current: Option<PathBuf>, argument: OsString) -> Result<PathBuf, Error> {
    match current {
        Some(_) => Err(Error::TooManyDirectories),
        None => Ok(PathBuf::from(argument)),
    }
}

/// Filter groups always contain at least the default "all files" group.
fn normalized_filters(filters: Vec<Filter>) -> Vec<Filter> {
    if filters.is_empty() {
        vec![Filter::all_files()]
    } else {
        filters
    }
}

/// Parsed command line.
#[derive(Clone, Debug)]
pub struct Arguments {
    /// Start directory, kept as a raw path so non-UTF-8 names survive.
    pub directory: Option<PathBuf>,
    /// Present only when `--pick-file` was given.
    pub picker: Option<Options>,
    pub help: bool,
    pub version: bool,
}

/// Picker behaviour handed to the browser.
#[derive(Clone, Debug)]
pub struct Options {
    /// Selectable filter groups; never empty.
    pub filters: Vec<Filter>,
    /// Initial read-only flag returned to the caller; it does not change file permissions.
    pub read_only: bool,
}

impl Options {
    /// Builds options, adding the default "all files" group when `filters` is
    /// empty so the invariant on [`Options::filters`] holds.
    pub fn new(filters: Vec<Filter>, read_only: bool) -> Self {
        Self {
            filters: normalized_filters(filters),
            read_only,
        }
    }

    /// Whether `path` is visible under the filter group at `filter_index`.
    ///
    /// Only the basename is matched; directories stay navigable because the
    /// browser never filters on this predicate for folders, and an unknown
    /// index hides every file.
    pub fn matches(&self, filter_index: usize, path: &Path) -> bool {
        self.filters
            .get(filter_index)
            .is_some_and(|filter| filter.is_match(path))
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::new(Vec::new(), false)
    }
}

/// One switchable filter group.
#[derive(Clone, Debug)]
pub struct Filter {
    /// `None` means the browser supplies its localized "all files" label;
    /// a bare glob without a label keeps the original glob text as label.
    pub label: Option<String>,
    matcher: GlobSet,
}

impl Filter {
    /// The group that shows every entry, used when no `--filter` was given.
    pub fn all_files() -> Self {
        Self::compile(None, &["*"], "*").expect("the wildcard glob is always valid")
    }

    /// Parses `[LABEL:]GLOB[,GLOB...]`.
    ///
    /// # Errors
    /// Reports empty or invalid glob patterns.
    pub fn from_spec(spec: &str) -> Result<Self, Error> {
        let (label, patterns) = match spec.split_once(':') {
            Some((label, patterns)) => (
                Some(label.trim()).filter(|label| !label.is_empty()),
                patterns,
            ),
            None => (Some(spec.trim()).filter(|label| !label.is_empty()), spec),
        };
        let patterns: Vec<&str> = patterns.split(',').map(str::trim).collect();
        Self::compile(label.map(str::to_owned), &patterns, spec)
    }

    fn compile(label: Option<String>, patterns: &[&str], spec: &str) -> Result<Self, Error> {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            if pattern.is_empty() {
                return Err(Error::EmptyFilter {
                    filter: spec.to_owned(),
                });
            }
            let glob = Glob::new(pattern).map_err(|source| Error::InvalidGlob {
                filter: spec.to_owned(),
                pattern: (*pattern).to_owned(),
                source,
            })?;
            builder.add(glob);
        }
        let matcher = builder.build().map_err(|source| Error::InvalidGlob {
            filter: spec.to_owned(),
            pattern: patterns.join(","),
            source,
        })?;
        Ok(Self { label, matcher })
    }

    /// Matches the raw basename, so invalid UTF-8 names are still comparable.
    fn is_match(&self, path: &Path) -> bool {
        let name = path.file_name().map_or(path, Path::new);
        self.matcher.is_match(name)
    }
}

/// Writes the confirmed selection to stdout as one JSON line and flushes it.
///
/// Nothing else in the picker writes to stdout. The file itself is neither
/// opened nor modified.
///
/// # Errors
/// Reports a path that cannot be made absolute, a serialization failure, or a
/// failed stdout write.
pub fn emit_selection(path: &Path, read_only: bool) -> Result<(), Error> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::absolute(path).map_err(|source| Error::AbsolutePath {
            path: path.to_path_buf(),
            source,
        })?
    };
    let selection = Selection {
        path: absolute.to_str(),
        path_os: absolute
            .to_str()
            .is_none()
            .then(|| absolute.clone().into_os_string()),
        read_only,
    };
    let line = serde_json::to_vec(&selection).map_err(Error::Serialize)?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&line).map_err(Error::Output)?;
    stdout.write_all(b"\n").map_err(Error::Output)?;
    stdout.flush().map_err(Error::Output)
}

/// The selection line. A non-UTF-8 path is reported as `path: null` plus
/// `path_os`, whose Serde representation keeps the raw bytes recoverable
/// instead of substituting a lossy string.
#[derive(Serialize)]
struct Selection<'a> {
    path: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_os: Option<OsString>,
    read_only: bool,
}

/// Records that the user picked a file and the result reached stdout.
pub fn mark_confirmed() {
    EXIT.store(EXIT_CONFIRMED, Ordering::Relaxed);
}

/// Records that the user cancelled or closed the window. This is the default.
pub fn mark_cancelled() {
    EXIT.store(EXIT_CANCELLED, Ordering::Relaxed);
}

/// Records that the selection could not be reported.
pub fn mark_failed() {
    EXIT.store(EXIT_FAILED, Ordering::Relaxed);
}

/// Exit status of the picker session.
pub fn exit_code() -> u8 {
    EXIT.load(Ordering::Relaxed)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown argument {argument:?}")]
    UnknownArgument { argument: OsString },
    #[error("the {option} option requires a value")]
    MissingValue { option: &'static str },
    #[error("{option} is only valid together with --pick-file")]
    PickerOnlyOption { option: &'static str },
    #[error("expected at most one directory argument")]
    TooManyDirectories,
    #[error("the --filter value must be valid UTF-8")]
    NonUtf8Filter,
    #[error("the filter {filter:?} has no glob pattern")]
    EmptyFilter { filter: String },
    #[error("invalid glob {pattern:?} in filter {filter:?}")]
    InvalidGlob {
        filter: String,
        pattern: String,
        #[source]
        source: globset::Error,
    },
    #[error("cannot determine the absolute path of {path:?}")]
    AbsolutePath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot serialize the selection")]
    Serialize(#[source] serde_json::Error),
    #[error("cannot write the selection to stdout")]
    Output(#[source] std::io::Error),
}

/// Mutable accumulators for the option flags.
#[derive(Default)]
struct Requested {
    /// `Some` once a picker-only flag appeared; holds the flag name for the
    /// error raised when `--pick-file` is missing.
    read_only: Option<&'static str>,
    filters: Vec<Filter>,
    picker: bool,
    help: bool,
    version: bool,
}
