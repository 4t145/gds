//! Live theme refresh while the application runs.
//!
//! Owns the DankMaterialShell export watcher and the platform appearance
//! observer. Both feed [`HotReload::request`], which performs one background
//! parse per event and installs the result on the UI thread.

use std::path::{Path, PathBuf};

use async_channel::{Receiver, Sender};
use gpui_kit::{AppContext, Context, Subscription, Task, Window, WindowAppearance};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use super::{Appearance, Error, Loaded, ModePreference, Source, apply_loaded, dms_colors_path};

/// Keeps the applied palette in sync with the desktop without touching saved
/// settings.
pub struct HotReload {
    /// The user's preference; a refresh never rewrites it.
    appearance: Appearance,
    /// Latest platform appearance, reported by the owning window.
    window_appearance: WindowAppearance,
    /// Bumped per request so a stale background parse cannot overwrite a newer
    /// selection.
    generation: u64,
    sender: Sender<()>,
    watcher: Option<DmsWatch>,
    load: Option<Task<()>>,
    _platform: Subscription,
    _events: Task<()>,
}

impl HotReload {
    /// Subscribe to platform appearance changes and, for the DMS source, to the
    /// export file. The caller applies the initial appearance itself.
    pub fn new(appearance: Appearance, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let platform = cx.observe_window_appearance(window, |this, window, cx| {
            this.window_appearance = window.appearance();
            if this.follows_platform() {
                this.request(cx);
            }
        });
        let (sender, receiver) = async_channel::unbounded();
        let events = Self::spawn_listener(receiver, cx);
        let mut this = Self {
            appearance,
            window_appearance: window.appearance(),
            generation: 0,
            sender,
            watcher: None,
            load: None,
            _platform: platform,
            _events: events,
        };
        this.sync_watch();
        this
    }

    /// Adopt applied preferences; opacity-only changes do not reread the palette.
    pub fn set_appearance(
        &mut self,
        appearance: Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reload =
            self.appearance.source != appearance.source || self.appearance.mode != appearance.mode;
        self.appearance = appearance;
        self.window_appearance = window.appearance();
        self.sync_watch();
        if reload {
            self.request(cx);
        }
    }

    /// Builtin and Qt sources derive their mode from the platform; DMS keeps the
    /// mode recorded in its export.
    fn follows_platform(&self) -> bool {
        self.appearance.mode == ModePreference::System && self.appearance.source != Source::Dms
    }

    /// Start or stop the export watcher to match the selected source.
    fn sync_watch(&mut self) {
        if self.appearance.source == Source::Dms {
            if self.watcher.is_none() {
                self.watcher = DmsWatch::start(self.sender.clone());
            }
        } else {
            self.watcher = None;
        }
    }

    fn spawn_listener(receiver: Receiver<()>, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            while receiver.recv().await.is_ok() {
                if this.update(cx, |this, cx| this.request(cx)).is_err() {
                    break;
                }
            }
        })
    }

    /// Reload from the current preference, cancelling any in-flight parse.
    fn request(&mut self, cx: &mut Context<Self>) {
        if let Some(watcher) = &mut self.watcher {
            watcher.retarget();
        }
        self.generation += 1;
        let generation = self.generation;
        let appearance = self.appearance.clone();
        let window_appearance = self.window_appearance;
        self.load = Some(cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_spawn(async move { appearance.load_for(window_appearance) })
                .await;
            this.update(cx, |this, cx| this.accept(generation, loaded, cx))
                .ok();
        }));
    }

    /// Install a background result unless a newer request superseded it.
    fn accept(&mut self, generation: u64, result: Result<Loaded, Error>, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        if let Err(error) = result.and_then(|loaded| apply_loaded(&self.appearance, loaded, cx)) {
            eprintln!("theme refresh failed, keeping the last applied theme: {error}");
        }
    }
}

/// Watches the directory holding the DMS export so atomic replacements of the
/// file are seen, not just in-place writes.
struct DmsWatch {
    target: PathBuf,
    watched: PathBuf,
    watcher: RecommendedWatcher,
}

impl DmsWatch {
    fn start(sender: Sender<()>) -> Option<Self> {
        let target = dms_colors_path()?;
        let watched = Self::containing_directory(&target)?;
        let filter = target.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: Result<Event, notify::Error>| {
                let event = match event {
                    Ok(event) => event,
                    Err(error) => {
                        eprintln!("theme watcher error: {error}");
                        return;
                    }
                };
                if is_theme_write(&event, &filter) {
                    // TODO: coalesce writes into one reload (~150 ms) instead of
                    // reloading once per write event.
                    // A closed channel means the owning HotReload is gone.
                    let _ = sender.try_send(());
                }
            })
            .inspect_err(|error| eprintln!("cannot create theme watcher: {error}"))
            .ok()?;
        watcher
            .watch(&watched, RecursiveMode::NonRecursive)
            .inspect_err(|error| {
                eprintln!(
                    "cannot watch theme directory {}: {error}",
                    watched.display()
                )
            })
            .ok()?;
        Some(Self {
            target,
            watched,
            watcher,
        })
    }

    /// Nearest existing ancestor, so a not-yet-created export directory can be
    /// watched for its creation.
    fn containing_directory(target: &Path) -> Option<PathBuf> {
        target
            .parent()?
            .ancestors()
            .find(|dir| dir.is_dir())
            .map(Path::to_owned)
    }

    /// Follow the export back into a directory created after startup.
    fn retarget(&mut self) {
        let Some(dir) = Self::containing_directory(&self.target) else {
            return;
        };
        if dir == self.watched {
            return;
        }
        if self
            .watcher
            .watch(&dir, RecursiveMode::NonRecursive)
            .is_ok()
        {
            let _ = self.watcher.unwatch(&self.watched);
            self.watched = dir;
        }
    }
}

/// Ignore access events: our own read of the export would otherwise feed the
/// watcher in a loop. Ancestor events count too, because the watched directory
/// may be the nearest existing one rather than the export's own parent.
fn is_theme_write(event: &Event, target: &Path) -> bool {
    let writes = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    );
    writes
        && event
            .paths
            .iter()
            .any(|path| path == target || target.starts_with(path))
}
