//! File clipboard interchange; Wayland uses real file MIME offers.
//! Other platforms retain a clearly limited GPUI same-process fallback.

mod cut;
mod fallback;
mod protocol;
#[cfg(target_os = "linux")]
mod wayland;

use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

use gpui_kit::{App, Task};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Copy,
    Cut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity(Option<String>);

impl Identity {
    fn fresh() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(1);
        Self(Some(format!(
            "gds:{}:{:?}:{}",
            std::process::id(),
            SystemTime::now(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        )))
    }

    fn is_owned(&self) -> bool {
        self.0.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub mode: Mode,
    pub paths: Vec<PathBuf>,
    pub identity: Identity,
}

impl Snapshot {
    /// # Errors
    /// Rejects empty selections and paths that are not absolute named filesystem entries.
    fn new(mode: Mode, paths: Vec<PathBuf>) -> Result<Self, Error> {
        let paths = Self::validate_paths(paths)?;
        Ok(Self {
            mode,
            paths,
            identity: Identity::fresh(),
        })
    }

    /// # Errors
    /// Rejects empty or non-local filesystem entry paths.
    fn validate_paths(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, Error> {
        let mut seen = HashSet::new();
        let mut unique = Vec::with_capacity(paths.len());
        for path in paths {
            if !path.is_absolute()
                || !matches!(path.components().next_back(), Some(Component::Normal(_)))
                || path.as_os_str().as_encoded_bytes().contains(&0)
            {
                return Err(Error::InvalidPath(path));
            }
            if seen.insert(path.clone()) {
                unique.push(path);
            }
        }
        if unique.is_empty() {
            return Err(Error::EmptySelection);
        }
        Ok(unique)
    }

    fn remaining_after(&self, current: &Self, succeeded: &[PathBuf]) -> Option<Vec<PathBuf>> {
        // Foreign offers have no per-copy identity. Do not overwrite their owner,
        // even if another copy happens to contain exactly the same path list.
        if self.mode != Mode::Cut || !self.identity.is_owned() || self != current {
            return None;
        }
        let completed: HashSet<_> = succeeded.iter().collect();
        let remaining: Vec<_> = self
            .paths
            .iter()
            .filter(|path| !completed.contains(path))
            .cloned()
            .collect();
        (remaining.len() != self.paths.len()).then_some(remaining)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no files are selected")]
    EmptySelection,
    #[error("clipboard path is not an absolute named filesystem entry: {0}")]
    InvalidPath(PathBuf),
    #[error("clipboard contains an unsupported or non-local file URI: {0}")]
    InvalidUri(String),
    #[error("invalid file clipboard data: {0}")]
    InvalidPayload(String),
    #[error("file clipboard changed while its formats were being read")]
    Changed,
    #[error("file clipboard payload exceeds the supported size")]
    TooLarge,
    #[error("timed out waiting for file clipboard data")]
    Timeout,
    #[error("file clipboard I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("file clipboard data could not be encoded or decoded: {0}")]
    Json(#[from] serde_json::Error),
    #[error("platform clipboard read failed: {0}")]
    PlatformRead(#[from] gpui_kit::ClipboardReadError),
    #[cfg(target_os = "linux")]
    #[error("Wayland clipboard read failed: {0}")]
    WaylandRead(#[from] wl_clipboard_rs::paste::Error),
    #[cfg(target_os = "linux")]
    #[error("Wayland clipboard write failed: {0}")]
    WaylandWrite(#[from] wl_clipboard_rs::copy::Error),
}

#[derive(Clone, Copy, Debug)]
enum Backend {
    #[cfg(target_os = "linux")]
    Wayland,
    Gpui,
}

#[derive(Clone, Copy, Debug)]
pub struct Bridge {
    backend: Backend,
}

impl Bridge {
    pub fn new(_: &App) -> Self {
        #[cfg(target_os = "linux")]
        if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
            || std::env::var_os("WAYLAND_SOCKET").is_some_and(|value| !value.is_empty())
        {
            return Self {
                backend: Backend::Wayland,
            };
        }
        Self {
            backend: Backend::Gpui,
        }
    }

    /// Start on the GPUI thread. Wayland connection and payload work run in the background.
    ///
    /// # Errors
    /// The task reports invalid paths, encoding or unavailable clipboard protocols.
    pub fn write(
        &self,
        mode: Mode,
        paths: Vec<PathBuf>,
        cx: &mut App,
    ) -> Task<Result<Snapshot, Error>> {
        match self.backend {
            #[cfg(target_os = "linux")]
            Backend::Wayland => cx.background_executor().spawn(async move {
                let snapshot = Snapshot::new(mode, paths)?;
                wayland::Clipboard::write(&snapshot)?;
                Ok(snapshot)
            }),
            Backend::Gpui => Task::ready(fallback::Clipboard::write(mode, paths, cx)),
        }
    }

    /// Read supported file clipboard formats, never infer files from ordinary text.
    ///
    /// # Errors
    /// The task reports malformed, non-local or inaccessible file clipboard data.
    pub fn read(&self, cx: &mut App) -> Task<Result<Option<Snapshot>, Error>> {
        match self.backend {
            #[cfg(target_os = "linux")]
            Backend::Wayland => cx
                .background_executor()
                .spawn(async { wayland::Clipboard::read() }),
            Backend::Gpui => fallback::Clipboard::read(cx),
        }
    }

    /// Remove completed cut entries only from an unchanged GDS-owned snapshot.
    /// Foreign owners and newly copied contents are left untouched. The Wayland
    /// protocol exposes no atomic compare-and-set; ownership is reread immediately
    /// before publishing a replacement, rather than assuming the old offer persists.
    ///
    /// # Errors
    /// The task reports clipboard access or update failures; filesystem results are unaffected.
    pub fn finish_cut(
        &self,
        snapshot: Snapshot,
        succeeded: Vec<PathBuf>,
        cx: &mut App,
    ) -> Task<Result<(), Error>> {
        if snapshot.mode != Mode::Cut || !snapshot.identity.is_owned() || succeeded.is_empty() {
            return Task::ready(Ok(()));
        }
        match self.backend {
            #[cfg(target_os = "linux")]
            Backend::Wayland => cx
                .background_executor()
                .spawn(async move { wayland::Clipboard::finish_cut(&snapshot, &succeeded) }),
            Backend::Gpui => fallback::Clipboard::finish_cut(snapshot, succeeded, cx),
        }
    }
}

/// Whether `path` is one of the sources of the current in-application cut.
///
/// This reads only the cached intent; no clipboard or filesystem I/O happens
/// during rendering. The cache changes only when a clipboard operation
/// completes or [`refresh_cut`] validates it.
pub fn is_cut(path: &Path, cx: &App) -> bool {
    cx.try_global::<cut::CutCache>()
        .is_some_and(|cache| cache.is_cut(path))
}

/// Re-read the clipboard and forget the cached cut markers if another
/// application or a later copy replaced the cut snapshot.
///
/// Callers invoke this on window activation or on a returning file-row click;
/// it never polls. One read is in flight at a time, and a read that started
/// before a newer copy/cut cannot replace that newer intent.
pub fn refresh_cut(cx: &mut App) {
    let Some((expected, generation)) = cx.default_global::<cut::CutCache>().begin_refresh() else {
        return;
    };
    let bridge = Bridge::new(cx);
    let read = bridge.read(cx);
    cx.spawn(async move |cx| {
        let current = read.await;
        let changed = cx.update(|cx| {
            cx.default_global::<cut::CutCache>()
                .reconcile(generation, &expected, current)
        });
        if changed {
            cx.update(|cx| cx.refresh_windows());
        }
    })
    .detach();
}

/// Forget the cached cut intent because another clipboard payload replaced it.
pub fn clear_cut(cx: &mut App) {
    let changed = {
        let cache = cx.default_global::<cut::CutCache>();
        let had_cut = cache.has_cut();
        cache.clear();
        had_cut
    };
    if changed {
        cx.refresh_windows();
    }
}

/// Record a clipboard write only after it actually succeeded.
pub(crate) fn note_clipboard_write(written: &Snapshot, cx: &mut App) {
    cx.default_global::<cut::CutCache>().record_write(written);
    cx.refresh_windows();
}

/// Drop the sources a cut paste moved, keeping any entry it did not move.
pub(crate) fn note_cut_paste(pasted: &Snapshot, succeeded: &[PathBuf], cx: &mut App) {
    if pasted.mode != Mode::Cut || succeeded.is_empty() {
        return;
    }
    cx.default_global::<cut::CutCache>()
        .note_pasted(pasted, succeeded);
    cx.refresh_windows();
}

#[cfg(test)]
mod tests;
