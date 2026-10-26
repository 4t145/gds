//! Cached cut intent for row presentation.
//!
//! Rendering only reads this cache; the clipboard itself is touched by the
//! clipboard operations and by [`super::refresh_cut`]'s asynchronous check.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use gpui_kit::Global;

use super::{Error, Mode, Snapshot};

#[derive(Default)]
pub(super) struct CutCache {
    /// The GDS-owned snapshot the cache was established from. Its identity may
    /// be stale after a partial cut paste republishes the same remaining paths.
    snapshot: Option<Snapshot>,
    cut_paths: HashSet<PathBuf>,
    /// Incremented whenever the cached intent changes, so a read that started
    /// before a newer copy/cut cannot overwrite it.
    generation: u64,
    /// A validation read is in flight for the intent of `generation`.
    refreshing: bool,
}

impl Global for CutCache {}

impl CutCache {
    pub(super) fn is_cut(&self, path: &Path) -> bool {
        self.cut_paths.contains(path)
    }

    pub(super) fn has_cut(&self) -> bool {
        self.snapshot.is_some() || !self.cut_paths.is_empty()
    }

    /// Adopt the intent of a clipboard write that this application completed.
    pub(super) fn record_write(&mut self, written: &Snapshot) {
        self.generation = self.generation.wrapping_add(1);
        self.refreshing = false;
        if written.mode == Mode::Cut {
            self.snapshot = Some(written.clone());
            self.cut_paths = written.paths.iter().cloned().collect();
        } else {
            self.snapshot = None;
            self.cut_paths.clear();
        }
    }

    pub(super) fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.refreshing = false;
        self.snapshot = None;
        self.cut_paths.clear();
    }

    /// Drop moved sources; keep the cut intent for sources that were not moved.
    ///
    /// A cache established by an older copy is left untouched: the clipboard
    /// now belongs to someone else even if these paths were moved by a paste.
    pub(super) fn note_pasted(&mut self, pasted: &Snapshot, succeeded: &[PathBuf]) {
        if pasted.mode != Mode::Cut {
            return;
        }
        let Some(current) = self.snapshot.as_mut() else {
            return;
        };
        if current.identity != pasted.identity {
            return;
        }
        let completed: HashSet<&PathBuf> = succeeded.iter().collect();
        let remaining: Vec<PathBuf> = current
            .paths
            .iter()
            .filter(|path| !completed.contains(path))
            .cloned()
            .collect();
        self.generation = self.generation.wrapping_add(1);
        // The changed intent needs its own validation; a read still in flight
        // for the previous one must not hold the single-flight flag.
        self.refreshing = false;
        self.cut_paths = remaining.iter().cloned().collect();
        if remaining.is_empty() {
            self.snapshot = None;
        } else {
            current.paths = remaining;
        }
    }

    /// Start one validation read, or `None` when no cut intent exists or a read
    /// is already in flight.
    pub(super) fn begin_refresh(&mut self) -> Option<(Snapshot, u64)> {
        if self.refreshing {
            return None;
        }
        let snapshot = self.snapshot.clone()?;
        self.refreshing = true;
        Some((snapshot, self.generation))
    }

    /// Apply a completed clipboard read if no newer copy/cut arrived meanwhile.
    ///
    /// Only a read that started for the current intent releases the
    /// single-flight flag; a stale completion must not let another read overlap
    /// the one that is already validating the current intent.
    ///
    /// Returns whether the rendered cut markers changed. A read error keeps the
    /// cache: it says nothing about what the clipboard currently holds.
    pub(super) fn reconcile(
        &mut self,
        generation: u64,
        expected: &Snapshot,
        current: Result<Option<Snapshot>, Error>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        self.refreshing = false;
        let replaced = match current {
            Ok(Some(found))
                if found.mode == Mode::Cut
                    && (found.identity == expected.identity || found.paths == expected.paths) =>
            {
                let changed = self.cut_paths != found.paths.iter().cloned().collect::<HashSet<_>>()
                    || self.snapshot.as_ref() != Some(&found);
                self.snapshot = Some(found.clone());
                self.cut_paths = found.paths.into_iter().collect();
                return changed;
            }
            Ok(_) => true,
            Err(_) => false,
        };
        let changed = replaced && self.has_cut();
        if replaced {
            self.generation = self.generation.wrapping_add(1);
            self.snapshot = None;
            self.cut_paths.clear();
        }
        changed
    }
}
