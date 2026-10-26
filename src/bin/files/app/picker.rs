//! Single-file selection mode.
//!
//! In this mode the browser stays a browser: folders open, tabs and locations
//! work, and the file operations the user already has are never intercepted.
//! What changes is only the outcome of choosing a file: the window reports the
//! path on stdout and closes instead of handing it to another program. The
//! report is decided here, never while rendering.

use std::sync::Arc;

use gds::ui::{self, IconButton, Shortcut};
use gds_uim::el;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
};
use gpui_kit::prelude::*;

use super::directory::{Entry, EntryKind, LinkStatus};
use super::*;

gpui_kit::actions!(gds_files_picker, [PickerConfirm, PickerCancel]);

/// Key context of the picker page.
///
/// The picker decisions live here, so `Alt+A` / `Alt+C` keep their dialog and
/// workspace meanings everywhere else.
pub(super) const PICKER_CONTEXT: &str = "Picker";

const CONTROL_GAP: f32 = 8.;

/// Toast slot of the picker's own validation messages.
pub(super) struct PickerNotice;

/// Register the picker decisions once per picker window.
///
/// Registered after the shared bindings, so within the picker scope these
/// strokes win only while [`PICKER_CONTEXT`] is on the focused path.
pub(super) fn register(cx: &mut App) {
    cx.bind_keys([
        binding("alt-a", Box::new(PickerConfirm)),
        binding("alt-c", Box::new(PickerCancel)),
    ]);
}

fn binding(stroke: &'static str, action: Box<dyn Action>) -> KeyBinding {
    let context = KeyBindingContextPredicate::parse(PICKER_CONTEXT)
        .expect("the bundled picker context is valid")
        .into();
    KeyBinding::load(
        stroke,
        action,
        Some(context),
        false,
        None,
        &DummyKeyboardMapper,
    )
    .expect("the bundled picker shortcuts are valid")
}

pub(super) fn confirm_shortcut() -> Shortcut {
    Shortcut::new(Box::new(PickerConfirm), Some(PICKER_CONTEXT))
}

pub(super) fn cancel_shortcut() -> Shortcut {
    Shortcut::new(Box::new(PickerCancel), Some(PICKER_CONTEXT))
}

/// The visible file projection of one filter group.
///
/// Every tab keeps a copy so a directory read projects its fresh entries
/// through the same filter instead of replacing a narrowed list with the whole
/// folder.
#[derive(Clone)]
pub(super) struct Projection {
    options: Arc<crate::picker::Options>,
    active_filter: usize,
}

impl Projection {
    /// Whether an entry belongs in the list: every folder stays visible and
    /// navigable, files have to match the active group.
    pub(super) fn keeps(&self, entry: &Entry) -> bool {
        Self::is_directory(entry) || self.options.matches(self.active_filter, &entry.path)
    }

    /// Whether the active group narrows files at all.
    pub(super) fn narrows(&self) -> bool {
        self.options
            .filters
            .get(self.active_filter)
            .is_some_and(|filter| filter.label.is_some())
    }

    /// Whether an entry is a folder, following a link only for navigation.
    pub(super) fn is_directory(entry: &Entry) -> bool {
        match entry.kind {
            EntryKind::Directory => true,
            EntryKind::SymbolicLink => entry.details.target.as_ref().is_some_and(|target| {
                target.status == LinkStatus::Resolved && target.kind == EntryKind::Directory
            }),
            _ => false,
        }
    }
}

/// One picker session.
pub(super) struct State {
    options: Arc<crate::picker::Options>,
    active_filter: usize,
    read_only: bool,
    /// Set once the selection is being written and the window is closing, so a
    /// second confirm can neither print twice nor reopen anything.
    completing: bool,
}

impl State {
    pub(super) fn new(options: crate::picker::Options) -> Self {
        Self {
            read_only: options.read_only,
            options: Arc::new(options),
            active_filter: 0,
            completing: false,
        }
    }

    pub(super) fn projection(&self) -> Projection {
        Projection {
            options: Arc::clone(&self.options),
            active_filter: self.active_filter,
        }
    }

    fn filters(&self) -> &[crate::picker::Filter] {
        &self.options.filters
    }

    fn filter_label(&self, index: usize, cx: &App) -> SharedString {
        self.filters()
            .get(index)
            .and_then(|filter| filter.label.clone())
            .map_or_else(|| i18n::text("picker-all-files", cx), |label| label.into())
    }

    fn matches(&self, path: &Path) -> bool {
        self.options.matches(self.active_filter, path)
    }

    fn select_filter(&mut self, index: usize) -> bool {
        if index == self.active_filter || index >= self.filters().len() {
            return false;
        }
        self.active_filter = index;
        true
    }
}

impl Browser {
    /// The one path the visible page has selected, regardless of what it is.
    fn selected_picker_path(&self) -> Option<PathBuf> {
        let slot = self.active()?;
        let tab = slot.tab.as_ref()?;
        let [path] = tab.navigation.view().selected.as_slice() else {
            return None;
        };
        Some(path.clone())
    }

    /// The one path a confirm would report, if the visible page holds exactly
    /// one selectable file.
    pub(super) fn picker_candidate(&self) -> Option<PathBuf> {
        let picker = self.picker.as_ref()?;
        if picker.completing {
            return None;
        }
        let path = self.selected_picker_path()?;
        if !picker.matches(&path) {
            return None;
        }
        let slot = self.active()?;
        let entry = slot.table_rows.iter().find(|entry| entry.path == path)?;
        (!Projection::is_directory(entry)).then_some(path)
    }

    pub(super) fn set_picker_filter(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        if index >= picker.filters().len() {
            return;
        }
        let projection = {
            let Some(picker) = self.picker.as_mut() else {
                return;
            };
            if !picker.select_filter(index) {
                return;
            }
            picker.projection()
        };
        self.reproject_picker(&projection);
        cx.notify();
    }

    pub(super) fn set_picker_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        let changed = self.picker.as_mut().is_some_and(|picker| {
            let changed = picker.read_only != read_only;
            picker.read_only = read_only;
            changed
        });
        if changed {
            cx.notify();
        }
    }

    /// Apply one projection to every file slot, dropping a selection the new
    /// group no longer shows.
    fn reproject_picker(&mut self, projection: &Projection) {
        for slot in std::iter::once(&mut self.home_tab).chain(self.tabs.iter_mut()) {
            slot.picker = Some(projection.clone());
            // A selection pending from a rename or a new folder is applied by
            // the refresh that follows it; consuming it now would drop it
            // before the entry exists.
            if slot.pending_selection.is_none() {
                slot.refresh_visible();
            }
        }
    }

    /// A click or Enter on a file row in picker mode.
    pub(super) fn activate_picker_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {
                self.navigate(path, cx);
                cx.notify();
            }
            Ok(metadata) if metadata.is_file() => self.complete_picker_file(Some(path), cx),
            Ok(_) => self.picker_notice("picker-not-file", None, cx),
            Err(error) => self.picker_notice("picker-missing", Some(error.to_string()), cx),
        }
    }

    /// Report the selection, then close the window.
    ///
    /// The path is revalidated here rather than trusted from the render state:
    /// the entry may have gone away or stopped being a regular file, and a
    /// stale projection must not let an out-of-group file through.
    pub(super) fn complete_picker_file(
        &mut self,
        candidate: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let read_only = match self.picker.as_ref() {
            Some(picker) if !picker.completing => picker.read_only,
            _ => return,
        };
        let Some(path) = candidate else {
            self.picker_notice("picker-select-file", None, cx);
            return;
        };
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => {
                self.picker_notice("picker-not-file", None, cx);
                return;
            }
            Err(error) => {
                self.picker_notice("picker-missing", Some(error.to_string()), cx);
                return;
            }
        }
        if !self
            .picker
            .as_ref()
            .is_some_and(|picker| picker.matches(&path))
        {
            self.picker_notice("picker-filter-mismatch", None, cx);
            return;
        }
        let written = crate::picker::emit_selection(&path, read_only);
        self.mark_picker_complete();
        match written {
            Ok(()) => crate::picker::mark_confirmed(),
            Err(error) => {
                eprintln!("Cannot report the picker selection: {error}");
                crate::picker::mark_failed();
            }
        }
        cx.quit();
    }

    /// Leave the picker without reporting anything.
    pub(super) fn cancel_picker(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        if picker.completing {
            return;
        }
        self.mark_picker_complete();
        crate::picker::mark_cancelled();
        cx.quit();
    }

    fn mark_picker_complete(&mut self) {
        if let Some(picker) = self.picker.as_mut() {
            picker.completing = true;
        }
    }

    fn picker_notice(&mut self, message_id: &str, detail: Option<String>, cx: &mut Context<Self>) {
        let mut report =
            notifications::Report::new(notifications::Level::Warning, i18n::text(message_id, cx));
        if let Some(detail) = detail {
            report = report.detail(detail);
        }
        report.replacing::<PickerNotice>().show(cx);
        cx.notify();
    }

    /// Confirm from the button or `Alt+A`.
    ///
    /// Local modes keep their own keys: a dialog, the menu, Settings or a rename
    /// editor must never reach out and close the window underneath it.
    pub(super) fn confirm_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker.is_none()
            || self.settings_open
            || self.menu_open
            || self.rename.read(cx).is_open()
            || window.has_active_dialog(cx)
        {
            return;
        }
        let candidate = self.selected_picker_path();
        self.complete_picker_file(candidate, cx);
    }

    pub(super) fn on_picker_confirm(
        &mut self,
        _: &PickerConfirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_picker(window, cx);
    }

    pub(super) fn on_picker_cancel(
        &mut self,
        _: &PickerCancel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_picker(cx);
    }

    pub(super) fn on_picker_confirm_click(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_picker(window, cx);
    }

    pub(super) fn on_picker_cancel_click(
        &mut self,
        _: &ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_picker(cx);
    }

    /// The footer controls a picker shows in place of the terminal toggle.
    pub(super) fn render_picker_controls(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(picker) = self.picker.as_ref() else {
            return el! { .div }.into_any_element();
        };
        let labels: Vec<SharedString> = (0..picker.filters().len())
            .map(|index| picker.filter_label(index, cx))
            .collect();
        let active_filter = picker.active_filter;
        let completion_blocked = picker.completing;
        let read_only = picker.read_only;
        let view = cx.entity().downgrade();

        let filter_view = view.clone();
        let filter = Button::new("picker-filter")
            .small()
            .label(picker.filter_label(active_filter, cx))
            .tooltip(i18n::text("picker-filter", cx))
            .dropdown_caret(true)
            .dropdown_menu(move |menu, _, _| {
                filter_menu(menu, labels.clone(), active_filter, filter_view.clone())
            });

        let read_only_button = Checkbox::new("picker-read-only")
            .small()
            .label(i18n::text("picker-read-only", cx))
            .checked(read_only);
        let read_view = view.clone();
        let read_only_button = read_only_button.on_change(move |checked, _, cx| {
            if let Some(view) = read_view.upgrade() {
                view.update(cx, |this, cx| this.set_picker_read_only(*checked, cx));
            }
        });

        let on_cancel = cx.listener(Self::on_picker_cancel_click);
        let cancel_shortcut = cancel_shortcut();
        let cancel = el! {
            "picker-cancel".IconButton::new {
                icon: IconName::X; label: $i18n::text("picker-cancel", cx);
                shortcut: cancel_shortcut; on_click: on_cancel;
            }
        };

        // A disabled decision has no key that could run it, so it stages no
        // hint; the reserved tail keeps the button from reflowing in between.
        let enabled = !completion_blocked && self.picker_candidate().is_some();
        let confirm_label = i18n::text("picker-confirm", cx);
        let confirm_shortcut = confirm_shortcut();
        let hint = if enabled {
            ui::keytips::suffix_anchor(
                window,
                cx,
                "picker-confirm-hint".into(),
                confirm_shortcut.clone(),
            )
        } else {
            el! { .div { size_0 } }.into_any_element()
        };
        let on_confirm = cx.listener(Self::on_picker_confirm_click);
        let confirm = Button::new("picker-confirm")
            .primary()
            .small()
            .icon(Icon::new(IconName::Check))
            .label(confirm_label.clone())
            .disabled(!enabled)
            .child(hint)
            .on_click(on_confirm);
        let confirm = el! {
            .div {
                # "picker-confirm-button"; flex_shrink_0;
                tooltip_show_delay: ui::TOOLTIP_DELAY;
                tooltip: $(ui::shortcut_tooltip(confirm_label, confirm_shortcut));
            } [confirm]
        };

        el! {
            .div {
                # "picker-controls"; flex; items_center; gap: CONTROL_GAP.px;
                flex_shrink_0; min_w_0;
            } [filter, read_only_button, cancel, confirm]
        }
        .into_any_element()
    }
}

/// The filter group rows, with the active group checked.
fn filter_menu(
    menu: PopupMenu,
    labels: Vec<SharedString>,
    active_filter: usize,
    view: WeakEntity<Browser>,
) -> PopupMenu {
    labels
        .into_iter()
        .enumerate()
        .fold(menu, |menu, (index, label)| {
            let target = view.clone();
            menu.item(
                PopupMenuItem::new(label)
                    .checked(index == active_filter)
                    .on_click(move |_, _, cx| {
                        if let Some(view) = target.upgrade() {
                            view.update(cx, |this, cx| this.set_picker_filter(index, cx));
                        }
                    }),
            )
        })
}
