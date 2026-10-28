//! Files' per-directory terminal collection, selection and tab-bar interactions.
//! Process startup and shutdown races belong to the single shared Terminal.

use std::path::PathBuf;

use gds::{
    i18n,
    terminal::{self, Terminal},
    ui,
};
use gpui_kit::{
    assets::IconName,
    component::{Disableable, Icon, Selectable, WindowExt, button::ButtonVariants},
    prelude::*,
    *,
};

use crate::actions::{
    CloseTerminal, FocusTerminal, NewTerminal, NextTerminal, PreviousTerminal,
    TERMINAL_TABS_CONTEXT,
};

#[cfg(all(test, feature = "gui-tests"))]
mod tests;

const TAB_HEIGHT: f32 = ui::SIDEBAR_ITEM_HEIGHT;

struct Entry {
    id: u64,
    terminal: Entity<Terminal>,
    closing: bool,
    _subscription: Subscription,
}

pub(in crate::app) struct Panel {
    entries: Vec<Entry>,
    active: Option<u64>,
    next_id: u64,
    tabs_focus: FocusHandle,
    cwd: Option<PathBuf>,
    preferences: terminal::Preferences,
    sidebar_width: f32,
    confirming: Option<u64>,
    sealed: bool,
}

impl Panel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            entries: Vec::new(),
            active: None,
            next_id: 0,
            tabs_focus: cx.focus_handle(),
            cwd: None,
            preferences: terminal::Preferences::default(),
            sidebar_width: ui::SIDEBAR_WIDTH,
            confirming: None,
            sealed: false,
        }
    }

    pub fn set_cwd(&mut self, cwd: Option<PathBuf>, cx: &mut Context<Self>) {
        let cwd = cwd.filter(|path| path.is_absolute());
        if self.cwd != cwd {
            self.cwd = cwd;
            cx.notify();
        }
    }

    pub fn set_sidebar_width(&mut self, width: f32, cx: &mut Context<Self>) {
        if width.is_finite() && self.sidebar_width != width {
            self.sidebar_width = width;
            cx.notify();
        }
    }

    pub fn set_preferences(&mut self, preferences: terminal::Preferences, cx: &mut Context<Self>) {
        if self.preferences == preferences {
            return;
        }
        self.preferences = preferences;
        for entry in &self.entries {
            entry.terminal.update(cx, |terminal, cx| {
                terminal.set_preferences(self.preferences.clone(), cx)
            });
        }
        cx.notify();
    }

    pub fn open(&mut self, cwd: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.sealed || !cwd.is_absolute() {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let preferences = self.preferences.clone();
        let terminal = cx.new(|cx| Terminal::new(cwd, preferences, window, cx));
        let subscription = cx.subscribe(&terminal, |_, _, _: &terminal::Event, cx| cx.notify());
        self.entries.push(Entry {
            id,
            terminal: terminal.clone(),
            closing: false,
            _subscription: subscription,
        });
        self.active = Some(id);
        terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
        cx.notify();
    }

    fn selected(&self) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| Some(entry.id) == self.active)
    }

    fn title(entry: &Entry, cx: &App) -> SharedString {
        let terminal = entry.terminal.read(cx);
        terminal
            .title(cx)
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| {
                terminal
                    .cwd()
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| terminal.cwd().display().to_string())
                    .into()
            })
    }

    pub fn active_title(&self, cx: &App) -> SharedString {
        self.selected()
            .map(|entry| Self::title(entry, cx))
            .unwrap_or_else(|| i18n::text("settings-terminal", cx))
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        if let Some(entry) = self.selected().filter(|entry| !entry.closing) {
            entry
                .terminal
                .update(cx, |terminal, cx| terminal.focus(window, cx));
        } else {
            self.tabs_focus.focus(window, cx);
        }
    }

    pub fn contains_focus(&self, window: &Window, cx: &App) -> bool {
        self.tabs_focus.contains_focused(window, cx)
            || self
                .entries
                .iter()
                .any(|entry| entry.terminal.read(cx).contains_focus(window, cx))
    }

    pub fn has_running_sessions(&self, cx: &App) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.terminal.read(cx).is_running(cx))
    }

    pub fn shutdown(&mut self, cx: &mut Context<Self>) -> terminal::Shutdown {
        self.sealed = true;
        self.confirming = None;
        let shutdowns: Vec<_> = self
            .entries
            .iter_mut()
            .map(|entry| {
                entry.closing = true;
                entry
                    .terminal
                    .update(cx, |terminal, cx| terminal.shutdown(cx))
            })
            .collect();
        cx.notify();
        // Closing entries remain owned until their cleanup completes. Repeated
        // collection shutdowns therefore wait on the same single-terminal work.
        Box::pin(async move {
            for shutdown in shutdowns {
                shutdown.await;
            }
        })
    }

    fn on_new(&mut self, _: &NewTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if !window.has_active_dialog(cx)
            && let Some(cwd) = self.cwd.clone()
        {
            self.open(cwd, window, cx);
        }
    }

    fn on_close(&mut self, _: &CloseTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.active {
            self.request_close(id, window, cx);
        }
    }

    fn on_focus(&mut self, _: &FocusTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if !window.has_active_dialog(cx) {
            self.focus(window, cx);
        }
    }

    fn on_next(&mut self, _: &NextTerminal, window: &mut Window, cx: &mut Context<Self>) {
        self.cycle(false, window, cx);
    }

    fn on_previous(&mut self, _: &PreviousTerminal, window: &mut Window, cx: &mut Context<Self>) {
        self.cycle(true, window, cx);
    }

    fn cycle(&mut self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.sealed || self.entries.is_empty() || window.has_active_dialog(cx) {
            return;
        }
        let current = self
            .entries
            .iter()
            .position(|entry| Some(entry.id) == self.active)
            .unwrap_or(0);
        let count = self.entries.len();
        let next = if backwards {
            (current + count - 1) % count
        } else {
            (current + 1) % count
        };
        self.select(self.entries[next].id, window, cx);
    }

    fn select(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.sealed || !self.entries.iter().any(|entry| entry.id == id) {
            return;
        }
        self.active = Some(id);
        self.tabs_focus.focus(window, cx);
        cx.notify();
    }

    fn request_close(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.sealed || self.confirming.is_some() || window.has_active_dialog(cx) {
            return;
        }
        let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.id == id && !entry.closing)
        else {
            return;
        };
        if !entry.terminal.read(cx).is_running(cx) {
            self.close_approved(id, window, cx);
            return;
        }
        self.confirming = Some(id);
        let owner = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let confirm = owner.clone();
            let cancel = owner.clone();
            let colors = gds::theme::Colors::current(cx);
            let mut background = colors.panel;
            background.a = 1.;
            dialog
                .confirm()
                .bg(background)
                .text_color(colors.panel_foreground)
                .title(i18n::text("files-terminal-end-title", cx))
                .description(i18n::text("files-terminal-end-message", cx))
                .footer(
                    ui::DialogActions::new(
                        i18n::text("files-terminal-end", cx),
                        IconName::X,
                        i18n::text("operation-cancel", cx),
                    )
                    .danger(),
                )
                .on_ok(move |_, window, cx| {
                    if let Err(error) =
                        confirm.update(cx, |panel, cx| panel.close_approved(id, window, cx))
                    {
                        eprintln!("Files terminal close owner disappeared: {error}");
                    }
                    true
                })
                .on_close(move |_, _, cx| {
                    if let Err(error) = cancel.update(cx, |panel, _| {
                        if panel.confirming == Some(id) {
                            panel.confirming = None;
                        }
                    }) {
                        eprintln!("Files terminal dialog owner disappeared: {error}");
                    }
                })
        });
    }

    fn close_approved(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.sealed {
            return;
        }
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == id && !entry.closing)
        else {
            return;
        };
        entry.closing = true;
        let shutdown = entry
            .terminal
            .update(cx, |terminal, cx| terminal.shutdown(cx));
        self.confirming = None;
        cx.spawn_in(window, async move |panel, cx| {
            shutdown.await;
            if let Err(error) =
                panel.update_in(cx, |panel, window, cx| panel.remove_stopped(id, window, cx))
            {
                eprintln!("Files terminal cleanup finished after its panel closed: {error}");
            }
        })
        .detach();
        cx.notify();
    }

    fn remove_stopped(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.sealed {
            return;
        }
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
            return;
        };
        let focused = self.contains_focus(window, cx);
        let was_active = self.active == Some(id);
        self.entries.remove(index);
        if was_active {
            self.active = self
                .entries
                .get(index.min(self.entries.len().saturating_sub(1)))
                .map(|entry| entry.id);
            if focused {
                self.tabs_focus.focus(window, cx);
            }
        }
        cx.notify();
    }
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = gds::theme::Colors::current(cx);
        let collapsed = self.sidebar_width <= ui::COLLAPSED_SIDEBAR_WIDTH + 1.;
        let mut tabs = div()
            .id("files-terminal-tabs")
            .w(px(self.sidebar_width))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .p(px(ui::SIDEBAR_PADDING))
            .gap_1()
            .bg(gds::theme::sidebar_background(cx))
            .text_color(colors.sidebar_foreground)
            .key_context(TERMINAL_TABS_CONTEXT)
            .track_focus(&self.tabs_focus)
            .on_action(cx.listener(Self::on_new))
            .on_action(cx.listener(Self::on_close))
            .on_action(cx.listener(Self::on_next))
            .on_action(cx.listener(Self::on_previous))
            .on_action(cx.listener(Self::on_focus));
        for entry in &self.entries {
            let id = entry.id;
            let terminal = entry.terminal.read(cx);
            let title = Self::title(entry, cx);
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("path", terminal.cwd().display().to_string());
            let tooltip: SharedString = format!(
                "{}\n{}",
                title,
                i18n::format("files-terminal-started-in", Some(&args), cx)
            )
            .into();
            let select =
                cx.listener(move |panel, _: &ClickEvent, window, cx| panel.select(id, window, cx));
            let close = cx.listener(move |panel, _: &ClickEvent, window, cx| {
                cx.stop_propagation();
                panel.request_close(id, window, cx);
            });
            let close_button = ui::button::Button::icon_button(("files-terminal-close", id))
                .control_icon(IconName::X)
                .tooltip(i18n::text("files-terminal-end", cx))
                .when(self.active == Some(id), |button| button.selected(true))
                .disabled(self.sealed || entry.closing)
                .on_click(close);
            tabs = tabs.child(
                div()
                    .id(("files-terminal-tab", id))
                    .flex()
                    .items_center()
                    .min_w_0()
                    .h(px(TAB_HEIGHT))
                    .flex_shrink_0()
                    .rounded(px(6.))
                    .pl_2()
                    .hover(|this| this.bg(colors.hover()))
                    .when(self.active == Some(id), |this| {
                        this.bg(colors.selection)
                            .text_color(colors.selection_foreground)
                    })
                    .on_click(select)
                    .tooltip(move |window, cx| ui::tooltip::view(tooltip.clone(), window, cx))
                    .when(collapsed, |this| {
                        this.justify_center()
                            .pl_0()
                            .child(Icon::new(IconName::Terminal).size(px(ui::SIDEBAR_ICON_SIZE)))
                    })
                    .when(!collapsed, |this| {
                        this.child(div().flex_1().min_w_0().truncate().text_sm().child(title))
                    })
                    .when(!collapsed, |this| this.child(close_button)),
            );
        }
        tabs = tabs.child(
            ui::button::Button::new("files-terminal-new")
                .ghost()
                .h(px(ui::ICON_BUTTON_SIZE))
                .w_full()
                .control_icon(IconName::Plus)
                .when(!collapsed, |button| {
                    button.label(i18n::text("files-terminal-new", cx))
                })
                .tooltip(i18n::text(
                    if self.cwd.is_some() {
                        "files-terminal-new"
                    } else {
                        "files-terminal-cwd-unavailable"
                    },
                    cx,
                ))
                .shortcut(ui::Shortcut::new(
                    Box::new(NewTerminal),
                    Some(TERMINAL_TABS_CONTEXT),
                ))
                .disabled(self.cwd.is_none() || self.sealed)
                .on_click(
                    cx.listener(|panel, _, window, cx| panel.on_new(&NewTerminal, window, cx)),
                ),
        );
        let body = match self.selected() {
            Some(entry) => entry.terminal.clone().into_any_element(),
            None => div()
                .size_full()
                .p_3()
                .bg(gds::theme::content_background(cx))
                .text_color(colors.muted_foreground)
                .child(i18n::text(
                    if self.cwd.is_some() {
                        "files-terminal-empty"
                    } else {
                        "files-terminal-cwd-unavailable"
                    },
                    cx,
                ))
                .into_any_element(),
        };
        div()
            .relative()
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .overflow_hidden()
            .child(tabs)
            .child(div().flex_1().min_h_0().min_w_0().h_full().child(body))
            .child(
                div()
                    .absolute()
                    .left(px(self.sidebar_width))
                    .top_0()
                    .bottom_0()
                    .w(px(1.))
                    .bg(colors.border),
            )
    }
}
