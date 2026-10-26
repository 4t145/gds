//! In-app file drags, drop targets, and drags from other applications.
//!
//! A drag carries original paths; the target decides the destination. Transfers
//! run through [`transfer::Batch`], so conflicts and self-moves keep the same
//! no-replace guarantees as paste.

use std::{collections::HashSet, path::Path, path::PathBuf, sync::Arc};

use gds_uim::el;
use gpui_kit::component::ActiveTheme;
use gpui_kit::{prelude::*, *};

use super::file_icon::FileIcon;
use super::*;
use crate::{
    directory::{Entry, EntryKind},
    operations::transfer,
};

mod spring;

use spring::{TabHover, TabHoverSpring};

const PREVIEW_PADDING_X: f32 = 10.;
const PREVIEW_PADDING_Y: f32 = 6.;
const PREVIEW_RADIUS: f32 = 6.;
const PREVIEW_MAX_WIDTH: f32 = 320.;
const PREVIEW_ICON_SIZE: f32 = 16.;
const PREVIEW_ICON_GAP: f32 = 8.;
/// Gap between the pointer and the preview chip's top-left corner.
const PREVIEW_POINTER_GAP: f32 = 12.;

/// Identity of the current in-app file drag, independent of reused row payloads.
#[derive(Default)]
struct FileDragSession(u64);
impl Global for FileDragSession {}

fn file_drag_session(cx: &App) -> u64 {
    cx.try_global::<FileDragSession>()
        .map_or(0, |session| session.0)
}

/// Invalidate any dwell left by an ended drag or an inactive window.
pub(super) fn cancel_tab_hover(cx: &mut App) {
    cx.set_global(FileDragSession(file_drag_session(cx).wrapping_add(1)));
}

/// One entry carried by a drag: its path, whether it is a directory, and the
/// icon the floating preview shows for it.
#[derive(Clone, Debug)]
struct DragItem {
    path: PathBuf,
    is_dir: bool,
    icon: FileIcon,
}

impl DragItem {
    fn new(entry: &Entry) -> Self {
        Self {
            path: entry.path.clone(),
            is_dir: entry.kind == EntryKind::Directory,
            icon: FileIcon::for_entry(entry),
        }
    }
}

/// Files carried by a drag that started in the file table.
#[derive(Clone, Debug)]
pub(super) struct FileDrag {
    entries: Arc<[DragItem]>,
}

impl FileDrag {
    fn shared(entries: Arc<[DragItem]>) -> Self {
        Self { entries }
    }

    fn single(entry: &Entry) -> Self {
        Self::shared(vec![DragItem::new(entry)].into())
    }

    fn paths(&self) -> Vec<PathBuf> {
        self.entries.iter().map(|item| item.path.clone()).collect()
    }

    fn count(&self) -> usize {
        self.entries.len()
    }

    fn preview_label(&self, cx: &App) -> SharedString {
        if let [item] = &self.entries[..] {
            let name = item.path.file_name().map_or_else(
                || item.path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            return name.into();
        }
        i18n::count("item-count", self.count(), cx)
    }

    fn preview_icon(&self) -> FileIcon {
        match self.entries.as_ref() {
            [item] => item.icon,
            _ => FileIcon::multiple(),
        }
    }

    /// Hand the real paths to the platform when the drag leaves the window.
    pub(super) fn external_payload(&self) -> ExternalDragPayload {
        ExternalDragPayload::Files(FileDragPaths::new(
            self.entries
                .iter()
                .map(|item| (item.path.clone(), item.is_dir)),
        ))
    }
}

/// Frozen selection for drag payloads. It is rebuilt only when the visible rows
/// or the selection change, so rendering a large virtual table never walks the
/// whole selection once per row.
#[derive(Default)]
pub(super) struct DragSnapshot {
    entries: Arc<[DragItem]>,
    selected: HashSet<PathBuf>,
}

impl DragSnapshot {
    pub(super) fn new(rows: &[Entry], selected: &[PathBuf]) -> Self {
        let selected: HashSet<PathBuf> = selected.iter().cloned().collect();
        let entries: Arc<[DragItem]> = rows
            .iter()
            .filter(|row| selected.contains(&row.path))
            .map(DragItem::new)
            .collect::<Vec<_>>()
            .into();
        Self { entries, selected }
    }

    /// A drag started on `entry` carries the whole selection when that row
    /// belongs to it, and only that row otherwise. Starting a drag never
    /// rewrites the selection, so an unselected row drags alone.
    pub(super) fn drag_for(&self, entry: &Entry) -> FileDrag {
        if self.selected.contains(&entry.path) {
            FileDrag::shared(Arc::clone(&self.entries))
        } else {
            FileDrag::single(entry)
        }
    }
}

/// The element that follows the pointer during an in-app file drag.
pub(super) struct DragPreview {
    icon: FileIcon,
    label: SharedString,
    /// Padding that moves the visible chip from the drag-start point to the pointer.
    content_offset: Point<Pixels>,
}

impl DragPreview {
    pub(super) fn new(drag: &FileDrag, cursor_offset: Point<Pixels>, cx: &mut App) -> Entity<Self> {
        cancel_tab_hover(cx);
        let icon = drag.preview_icon();
        let label = drag.preview_label(cx);
        // GPUI paints a drag preview at `pointer - cursor_offset`, where the offset
        // is the drag-start point inside the source row. Padding by that offset
        // again cancels it, so the chip sits next to the pointer wherever the drag
        // began instead of at the row origin.
        let content_offset =
            cursor_offset + point(px(PREVIEW_POINTER_GAP), px(PREVIEW_POINTER_GAP));
        cx.new(|_| Self {
            icon,
            label,
            content_offset,
        })
    }
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let icon = self.icon;
        let color = cx.theme().foreground;
        el! {
            .div { pl: $self.content_offset.x; pt: $self.content_offset.y; } [
                .div {
                    px: PREVIEW_PADDING_X.px; py: PREVIEW_PADDING_Y.px; rounded: PREVIEW_RADIUS.px;
                    flex; items_center; gap: PREVIEW_ICON_GAP.px;
                    max_w: PREVIEW_MAX_WIDTH.px; overflow_hidden;
                    bg: $cx.theme().secondary; border_1; border_color: $cx.theme().border;
                    text_color: $color; text_sm;
                } [
                    $(icon.render(px(PREVIEW_ICON_SIZE), color)),
                    .div { min_w_0; flex_1; truncate } [$(self.label.clone())],
                ],
            ]
        }
    }
}

/// Make `element` accept both in-app file drags and drags from other
/// applications, transferring into `destination`.
pub(super) fn drop_target(
    element: Stateful<Div>,
    destination: PathBuf,
    browser: WeakEntity<Browser>,
) -> Stateful<Div> {
    let in_app = {
        let browser = browser.clone();
        let destination = destination.clone();
        move |drag: &FileDrag, window: &mut Window, cx: &mut App| {
            if let Some(browser) = browser.upgrade() {
                browser.update(cx, |browser, cx| {
                    browser.drop_files(destination.clone(), drag, window, cx)
                });
            }
        }
    };
    let external = move |paths: &ExternalPaths, _: &mut Window, cx: &mut App| {
        if let Some(browser) = browser.upgrade() {
            browser.update(cx, |browser, cx| {
                browser.drop_external_files(destination.clone(), paths, cx)
            });
        }
    };
    element
        .on_drop(in_app)
        .on_drop(external)
        .drag_over::<FileDrag>(|style, _, _, cx| style.bg(cx.theme().accent.opacity(0.14)))
        .drag_over::<ExternalPaths>(|style, _, _, cx| style.bg(cx.theme().accent.opacity(0.14)))
}

/// Key of the one [`TabHoverSpring`] a window's tab strip shares. It is resolved
/// while the browser view renders, so every tab row reaches the same state.
const TAB_HOVER_KEY: &str = "tab-file-drop-hover";

/// A tab row that accepts file drops and switches to itself while a file drag
/// rests on it.
///
/// The drop transfer is exactly [`drop_target`]'s; the dwell only decides when
/// the row becomes the visible one. Platform drags from other applications carry
/// no moves inside this window, so only drags that started here can arm it.
pub(super) fn tab_drop_target(
    element: Stateful<Div>,
    destination: PathBuf,
    tab_id: u64,
    browser: WeakEntity<Browser>,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let spring = window.use_keyed_state(TAB_HOVER_KEY, cx, |_, _| TabHoverSpring::default());
    let element = drop_target(element, destination, browser.clone());
    let hover = {
        let spring = spring.clone();
        let browser = browser.clone();
        move |event: &DragMoveEvent<FileDrag>, window: &mut Window, cx: &mut App| {
            let Some(target) = browser.upgrade() else {
                return;
            };
            let browser_view = target.read(cx);
            let on_current = browser_view.active_id == tab_id && browser_view.collection.is_none();
            let hover = TabHover {
                session: file_drag_session(cx),
                tab_id,
                bounds: event.bounds,
                pointer: event.event.position,
                on_current,
            };
            let Some(generation) = spring.update(cx, |spring, _| spring.observe(hover)) else {
                return;
            };
            spring::wait_then_switch(
                spring.downgrade(),
                target.downgrade(),
                generation,
                window,
                cx,
            );
        }
    };
    element.on_drag_move::<FileDrag>(hover)
}

/// Sources that need no transfer: entries already inside the destination folder
/// and the destination folder itself. Dropping a selection back onto its own
/// folder must not run a redundant move or report conflicts.
fn transferable_sources(destination: &Path, sources: Vec<PathBuf>) -> Vec<PathBuf> {
    sources
        .into_iter()
        .filter(|source| source.parent() != Some(destination) && source.as_path() != destination)
        .collect()
}

impl Browser {
    fn can_accept_file_drop(&self) -> bool {
        !self.file_operation_in_progress && !self.settings_open && self.collection.is_none()
    }

    /// In-app drags move by default; holding the secondary modifier copies.
    fn drop_files(
        &mut self,
        destination: PathBuf,
        drag: &FileDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mode = if window.modifiers().secondary() {
            transfer::Mode::Copy
        } else {
            transfer::Mode::Move
        };
        self.transfer_dropped(drag.paths(), destination, mode, cx);
    }

    /// Platform drags carry no usable modifiers here, so they copy and never
    /// remove the originals.
    fn drop_external_files(
        &mut self,
        destination: PathBuf,
        paths: &ExternalPaths,
        cx: &mut Context<Self>,
    ) {
        self.transfer_dropped(
            paths.paths().to_vec(),
            destination,
            transfer::Mode::Copy,
            cx,
        );
    }

    fn transfer_dropped(
        &mut self,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        mode: transfer::Mode,
        cx: &mut Context<Self>,
    ) {
        if !self.can_accept_file_drop() {
            return;
        }
        if !self.device_path_accepts_task(&destination, cx)
            || sources
                .iter()
                .any(|path| !self.device_path_accepts_task(path, cx))
        {
            notifications::info(i18n::text("device-path-unavailable", cx), cx);
            return;
        }
        let paths = transferable_sources(&destination, sources);
        if paths.is_empty() {
            return;
        }
        let count = paths.len();
        let request = transfer::Request {
            paths,
            destination,
            mode,
        };
        let batch = match transfer::Batch::new(request) {
            Ok(batch) => batch,
            Err(error) => {
                notifications::Operation::finish(
                    notifications::Report::new(notifications::Level::Error, error.to_string()),
                    cx,
                );
                cx.notify();
                return;
            }
        };
        self.file_operation_in_progress = true;
        self.file_operation_paths = batch
            .paths()
            .iter()
            .cloned()
            .chain(std::iter::once(batch.destination().to_path_buf()))
            .collect();
        notifications::Operation::start(i18n::count("transfer-working", count, cx), cx);
        let work = cx
            .background_executor()
            .spawn(async move { batch.execute() });
        cx.spawn(async move |this, cx| {
            let report = work.await;
            if let Err(closed) = this.update(cx, |this, cx| {
                this.file_operation_in_progress = false;
                this.finish_transfer(report, None, cx);
            }) {
                eprintln!("Drop result arrived after the view closed: {closed}");
            }
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
