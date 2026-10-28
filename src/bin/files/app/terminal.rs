//! Files owns terminal lifetime at the directory-slot boundary, not the window.

mod panel;

use super::*;
use gpui_kit::{
    assets::IconName,
    component::{ActiveTheme, Icon},
    prelude::*,
};
use panel::Panel;

#[cfg(test)]
mod tests;

// Keep a tab, the optional volume group, and at least one location reachable.
pub(super) const MIN_CONTENT_HEIGHT: f32 = sidebar::MIN_TABS_HEIGHT
    + sidebar::MIN_DEVICES_HEIGHT
    + ui::split::DIVIDER_HEIGHT
    + ui::SIDEBAR_ITEM_HEIGHT * 2.;
pub(super) const MIN_DOCK_HEIGHT: f32 = 160.;
pub(super) const DEFAULT_DOCK_HEIGHT: f32 = 280.;
const CONFIRMATION_WIDTH: f32 = 460.;
const HEADER_HEIGHT: f32 = 36.;

pub(super) struct Dock {
    pub panel: Entity<Panel>,
    cwd: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloseTarget {
    Tab(u64),
    Window,
}

impl CloseTarget {
    fn includes(self, slot: u64) -> bool {
        self == Self::Window || self == Self::Tab(slot)
    }
}

/// A virtual, unmounted or relative location must never borrow Home's cwd.
fn local_cwd(path: &Path, available: bool) -> Option<PathBuf> {
    (available && path.is_absolute()).then(|| path.to_path_buf())
}

impl Browser {
    fn terminal_slots(&self) -> impl Iterator<Item = &OpenTab> {
        std::iter::once(&self.home_tab)
            .chain(self.tabs.iter())
            .chain(self.volume_pages.entries.iter().map(|page| &page.slot))
    }

    pub(super) fn terminal_visible(&self) -> bool {
        self.terminal_visible
            && self.picker.is_none()
            && !self.settings_open
            && self.active().is_some()
    }

    pub(super) fn terminal_focused(&self, window: &Window, cx: &App) -> bool {
        self.terminal_slots().any(|slot| {
            slot.terminal
                .as_ref()
                .is_some_and(|dock| dock.panel.read(cx).contains_focus(window, cx))
        })
    }

    fn terminal_cwd(&self, cx: &App) -> Option<PathBuf> {
        if self.picker.is_some() {
            return None;
        }
        let slot = self.active()?;
        local_cwd(
            slot.path(),
            slot.tab.is_some()
                && slot.files_available()
                && self.device_path_available(slot.path(), cx),
        )
    }

    /// Creating the empty panel is not creating a shell. Home and volume pages
    /// use exactly the same slot ownership as ordinary tabs.
    fn ensure_terminal(&mut self, cx: &mut Context<Self>) -> Option<Entity<Panel>> {
        if self.picker.is_some() {
            return None;
        }
        let slot = self.active()?;
        if self.terminal_closing
            && self
                .terminal_close_pending
                .is_some_and(|target| target.includes(slot.id))
        {
            return None;
        }
        if let Some(dock) = &slot.terminal {
            return Some(dock.panel.clone());
        }
        let owner = slot.id;
        let cwd = self.terminal_cwd(cx);
        let preferences = self.preferences.terminal.clone();
        let panel = cx.new(|cx| {
            let mut panel = Panel::new(cx);
            panel.set_preferences(preferences, cx);
            panel.set_cwd(cwd.clone(), cx);
            panel
        });
        self.tab_slot_mut(owner)?.terminal = Some(Dock {
            panel: panel.clone(),
            cwd,
        });
        Some(panel)
    }

    pub(super) fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = self.ensure_terminal(cx) else {
            return;
        };
        self.settings_open = false;
        self.reset_tab_prefix();
        self.path_editing = false;
        self.terminal_visible = !self.terminal_visible;
        if self.terminal_visible {
            panel.update(cx, |panel, cx| panel.focus(window, cx));
        } else {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    pub(super) fn open_internal_terminal(
        &mut self,
        cwd: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.picker.is_some() || self.settings_open || self.collection.is_some() {
            return;
        }
        let available = self.active().is_some_and(|slot| slot.files_available())
            && self.device_path_available(&cwd, cx);
        let Some(cwd) = local_cwd(&cwd, available) else {
            notifications::info(i18n::text("files-terminal-cwd-unavailable", cx), cx);
            return;
        };
        let Some(panel) = self.ensure_terminal(cx) else {
            return;
        };
        self.reset_tab_prefix();
        self.path_editing = false;
        self.terminal_visible = true;
        // The single terminal validates cwd on its background startup path. A failed
        // directory never changes this target or launches in a fallback folder.
        panel.update(cx, |panel, cx| panel.open(cwd, window, cx));
        cx.notify();
    }

    /// File navigation only changes the *next* new-session target. It never
    /// writes a command into the active PTY, even when the shell looks idle.
    pub(super) fn sync_terminal_cwd(&mut self, cx: &mut Context<Self>) {
        if self.terminal_visible() {
            self.ensure_terminal(cx);
        }
        let cwd = self.terminal_cwd(cx);
        let Some(dock) = self.active_mut().and_then(|slot| slot.terminal.as_mut()) else {
            return;
        };
        if dock.cwd != cwd {
            dock.cwd = cwd.clone();
            dock.panel.update(cx, |panel, cx| panel.set_cwd(cwd, cx));
        }
    }

    pub(super) fn on_return_from_terminal(
        &mut self,
        _: &ReturnFromTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx) || self.rename.read(cx).is_open() {
            return;
        }
        if !self.terminal_focused(window, cx) || self.settings_open {
            cx.propagate();
            return;
        }
        self.restore_terminal_content(window, cx);
    }

    fn restore_terminal_content(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_maximized = false;
        self.focus.focus(window, cx);
        Self::restore_file_focus(cx.entity().downgrade(), self.active_id, window);
        cx.notify();
    }

    fn maximize_terminal(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dock) = self.active().and_then(|slot| slot.terminal.as_ref()) else {
            return;
        };
        let panel = dock.panel.clone();
        self.terminal_maximized = !self.terminal_maximized;
        panel.update(cx, |panel, cx| panel.focus(window, cx));
        cx.notify();
    }

    pub(super) fn terminal_workspace(
        &self,
        workspace: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.terminal_visible() {
            return workspace;
        }
        let Some(dock) = self.active().and_then(|slot| slot.terminal.as_ref()) else {
            return workspace;
        };
        let panel = dock.panel.clone();
        let maximized = self.terminal_maximized;
        let maximize = ui::IconButton::new("files-terminal-maximize")
            .icon(if maximized {
                IconName::Minimize2
            } else {
                IconName::Maximize2
            })
            .label(i18n::text(
                if maximized {
                    "files-terminal-restore"
                } else {
                    "files-terminal-maximize"
                },
                cx,
            ))
            .on_click(cx.listener(Self::maximize_terminal));
        let hide = ui::IconButton::new("files-terminal-hide")
            .icon(IconName::ChevronDown)
            .label(i18n::text("files-terminal-hide", cx))
            .command(Command::Terminal)
            .on_click(cx.listener(Self::click_command(Command::Terminal)));
        let width = self.width();
        panel.update(cx, |panel, cx| panel.set_sidebar_width(width, cx));
        let frame = ui::Frame::new(width);
        let title = i18n::text("settings-terminal", cx);
        let heading = div()
            .id("files-terminal-title")
            .flex()
            .items_center()
            .h_full()
            .min_w_0()
            .px_2()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .when(self.sidebar_collapsed, |this| {
                let title = title.clone();
                this.justify_center()
                    .child(Icon::new(IconName::Terminal).size(px(ui::SIDEBAR_ICON_SIZE)))
                    .tooltip(move |window, cx| ui::tooltip::view(title.clone(), window, cx))
            })
            .when(!self.sidebar_collapsed, |this| {
                this.child(div().min_w_0().truncate().child(title))
            });
        let controls = div()
            .flex()
            .items_center()
            .justify_end()
            .h_full()
            .min_w_0()
            .px_2()
            .gap_2()
            .child(maximize)
            .child(hide);
        let header = frame
            .workspace(heading.into_any_element(), controls.into_any_element(), cx)
            .flex_none()
            .h(px(HEADER_HEIGHT));
        let dock = div()
            .id("files-terminal-dock")
            .test_support()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(panel),
            );
        if maximized {
            dock.into_any_element()
        } else {
            let divider = frame
                .workspace(div().into_any_element(), div().into_any_element(), cx)
                .flex_none()
                .size_full();
            ui::split::VerticalSplit::new(
                self.terminal_split.clone(),
                workspace,
                dock.into_any_element(),
                i18n::text("files-terminal-resize", cx),
            )
            .divider_background(divider)
            .into_any_element()
        }
    }

    pub(super) fn on_terminal_split(
        &mut self,
        _: Entity<ui::split::State>,
        event: &ui::split::Changed,
        cx: &mut Context<Self>,
    ) {
        self.preferences.files.terminal_content_height = Some(event.0);
        if let Err(error) = settings::save(&self.preferences) {
            report_settings_failure("settings-save-failed", &error, cx);
        }
    }

    pub(super) fn apply_terminal_preferences(&self, cx: &mut Context<Self>) {
        let panels: Vec<_> = self
            .terminal_slots()
            .filter_map(|slot| slot.terminal.as_ref().map(|dock| dock.panel.clone()))
            .collect();
        for panel in panels {
            panel.update(cx, |panel, cx| {
                panel.set_preferences(self.preferences.terminal.clone(), cx)
            });
        }
    }

    fn closing_panels(&self, target: CloseTarget) -> Vec<Entity<Panel>> {
        self.terminal_slots()
            .filter(|slot| target.includes(slot.id))
            .filter_map(|slot| slot.terminal.as_ref().map(|dock| dock.panel.clone()))
            .collect()
    }

    fn close_has_running_sessions(&self, target: CloseTarget, cx: &App) -> bool {
        self.closing_panels(target)
            .iter()
            .any(|panel| panel.read(cx).has_running_sessions(cx))
    }

    pub(super) fn on_close_window(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Client-side titlebar controls remove windows directly; route them
        // through the same confirmation and shutdown barrier as WM requests.
        self.request_terminal_close(CloseTarget::Window, window, cx);
    }

    pub(super) fn install_terminal_close_handler(window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            owner
                .update(cx, |this, cx| {
                    if this.terminal_close_pending.is_some() {
                        return false;
                    }
                    if this.closing_panels(CloseTarget::Window).is_empty() {
                        return true;
                    }
                    this.request_terminal_close(CloseTarget::Window, window, cx);
                    false
                })
                .unwrap_or(true)
        });
    }

    pub(super) fn request_terminal_close(
        &mut self,
        target: CloseTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.terminal_close_pending.is_some() || window.has_active_dialog(cx) {
            return;
        }
        if !self.close_has_running_sessions(target, cx) {
            self.finish_terminal_close(target, window, cx);
            return;
        }
        self.terminal_close_pending = Some(target);
        let owner = cx.entity().downgrade();
        // Let a triggering popup unmount before the confirmation captures focus.
        window.on_next_frame(move |window, cx| {
            let confirm = owner.clone();
            let cancel = owner.clone();
            window.open_alert_dialog(cx, move |dialog, _, cx| {
                let confirm = confirm.clone();
                let cancel = cancel.clone();
                let colors = gds::theme::Colors::current(cx);
                let mut background = colors.panel;
                background.a = 1.;
                dialog
                    .bg(background)
                    .text_color(colors.panel_foreground)
                    .confirm()
                    .width(px(CONFIRMATION_WIDTH))
                    .title(i18n::text("files-terminal-close-title", cx))
                    .description(i18n::text(
                        match target {
                            CloseTarget::Tab(_) => "files-terminal-close-tab-message",
                            CloseTarget::Window => "files-terminal-close-window-message",
                        },
                        cx,
                    ))
                    .footer(
                        ui::DialogActions::new(
                            i18n::text("files-terminal-terminate-close", cx),
                            IconName::X,
                            i18n::text("operation-cancel", cx),
                        )
                        .danger(),
                    )
                    .on_ok(move |_, window, cx| {
                        if let Err(error) = confirm.update(cx, |this, cx| {
                            if this.terminal_close_pending == Some(target) {
                                this.finish_terminal_close(target, window, cx);
                            }
                        }) {
                            eprintln!("Files close confirmation owner disappeared: {error}");
                        }
                        true
                    })
                    .on_close(move |_, _, cx| {
                        if let Err(error) = cancel.update(cx, |this, _| {
                            if !this.terminal_closing && this.terminal_close_pending == Some(target)
                            {
                                this.terminal_close_pending = None;
                            }
                        }) {
                            eprintln!("Files close dialog owner disappeared: {error}");
                        }
                    })
            });
        });
    }

    fn finish_terminal_close(
        &mut self,
        target: CloseTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.terminal_closing {
            return;
        }
        // Approval is the only path that mutates sessions or tab state. The
        // shutdown futures include in-flight spawns and worker cleanup, so the
        // final window cannot quit before the last PTY has actually stopped.
        self.terminal_closing = true;
        self.terminal_close_pending = Some(target);
        let shutdowns: Vec<_> = self
            .closing_panels(target)
            .into_iter()
            .map(|panel| panel.update(cx, |panel, cx| panel.shutdown(cx)))
            .collect();
        cx.spawn_in(window, async move |this, cx| {
            // Every shutdown already requested termination before waiting.
            for shutdown in shutdowns {
                shutdown.await;
            }
            if let Err(error) = this.update_in(cx, |this, window, cx| {
                this.terminal_closing = false;
                this.terminal_close_pending = None;
                match target {
                    CloseTarget::Tab(id) => this.close_tab_approved(id, window, cx),
                    CloseTarget::Window => window.remove_window(),
                }
            }) {
                eprintln!("Terminal shutdown finished after the Files window closed: {error}");
            }
        })
        .detach();
    }
}
