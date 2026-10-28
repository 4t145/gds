//! Agents owns terminal selection, management shortcuts and close policy.

use futures::{FutureExt as _, future::Shared};
use gds::terminal::{Preferences, Shutdown, Status, Terminal};
use gpui_kit::{
    App, AppContext, Context, Entity, FocusHandle, IntoElement, Render, SharedString, Subscription,
    TestSupportExt, Window,
    assets::IconName,
    component::{ActiveTheme, Disableable, Icon, Selectable, WindowExt, button::ButtonVariants},
    div,
    prelude::*,
    px,
};
use std::path::PathBuf;

use super::{
    CloseTerminal, FocusTerminal, NewTerminal, NextTerminal, PreviousTerminal, TABS_CONTEXT,
};
use crate::ui::Button;

#[cfg(all(test, feature = "gui-tests"))]
mod tests;

struct Entry {
    id: u64,
    terminal: Entity<Terminal>,
    _events: Subscription,
}

pub(in crate::workspace) struct Panel {
    entries: Vec<Entry>,
    active: Option<u64>,
    next_id: u64,
    cwd: Option<PathBuf>,
    preferences: Preferences,
    tabs_focus: FocusHandle,
    sidebar_width: f32,
    confirming: Option<u64>,
    closing: bool,
    retiring: Vec<Shared<Shutdown>>,
    shutdown: Option<Shared<Shutdown>>,
}

impl Panel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            entries: Vec::new(),
            active: None,
            next_id: 0,
            cwd: None,
            preferences: Preferences::default(),
            tabs_focus: cx.focus_handle(),
            sidebar_width: gds::ui::SIDEBAR_WIDTH,
            confirming: None,
            closing: false,
            retiring: Vec::new(),
            shutdown: None,
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
        if self.sidebar_width != width {
            self.sidebar_width = width;
            cx.notify();
        }
    }

    pub fn set_preferences(&mut self, preferences: Preferences, cx: &mut Context<Self>) {
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
        if self.closing || !cwd.is_absolute() {
            return;
        }
        let terminal = cx.new(|cx| Terminal::new(cwd, self.preferences.clone(), window, cx));
        let events = cx.subscribe(&terminal, |_, _, _: &gds::terminal::Event, cx| cx.notify());
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(Entry {
            id,
            terminal: terminal.clone(),
            _events: events,
        });
        self.active = Some(id);
        terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
        cx.notify();
    }

    fn active_terminal(&self) -> Option<&Entity<Terminal>> {
        self.entries
            .iter()
            .find(|entry| Some(entry.id) == self.active)
            .map(|entry| &entry.terminal)
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        if let Some(terminal) = self.active_terminal() {
            terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
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

    #[cfg(all(test, feature = "gui-tests"))]
    pub fn has_sessions(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn has_running_sessions(&self, cx: &App) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.terminal.read(cx).is_running(cx))
    }

    pub fn shutdown(&mut self, cx: &mut Context<Self>) -> Shutdown {
        if let Some(shutdown) = &self.shutdown {
            return Box::pin(shutdown.clone());
        }
        self.closing = true;
        self.confirming = None;
        self.active = None;
        let mut pending = std::mem::take(&mut self.retiring);
        for entry in self.entries.drain(..) {
            pending.push(
                entry
                    .terminal
                    .update(cx, |terminal, cx| terminal.shutdown(cx))
                    .shared(),
            );
        }
        let shutdown: Shutdown = Box::pin(async move {
            for pending in pending {
                pending.await;
            }
        });
        let shutdown = shutdown.shared();
        cx.background_executor().spawn(shutdown.clone()).detach();
        self.shutdown = Some(shutdown.clone());
        cx.notify();
        Box::pin(shutdown)
    }

    fn new_terminal(&mut self, _: &NewTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(cwd) = self.cwd.clone() {
            self.open(cwd, window, cx);
        }
    }

    fn close_terminal(&mut self, _: &CloseTerminal, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.active {
            self.request_close(id, window, cx);
        }
    }

    fn focus_terminal(&mut self, _: &FocusTerminal, window: &mut Window, cx: &mut Context<Self>) {
        self.focus(window, cx);
    }

    fn next_terminal(&mut self, _: &NextTerminal, window: &mut Window, cx: &mut Context<Self>) {
        self.cycle(false, window, cx);
    }

    fn previous_terminal(
        &mut self,
        _: &PreviousTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cycle(true, window, cx);
    }

    fn cycle(&mut self, reverse: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.entries.is_empty() || self.closing {
            return;
        }
        let index = self
            .entries
            .iter()
            .position(|entry| Some(entry.id) == self.active)
            .unwrap_or(0);
        let count = self.entries.len();
        self.active = Some(self.entries[(index + if reverse { count - 1 } else { 1 }) % count].id);
        self.tabs_focus.focus(window, cx);
        cx.notify();
    }

    fn remove(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
            return;
        };
        let entry = self.entries.remove(index);
        let pending = entry
            .terminal
            .update(cx, |terminal, cx| terminal.shutdown(cx))
            .shared();
        cx.background_executor().spawn(pending.clone()).detach();
        self.retiring.retain(|pending| pending.peek().is_none());
        self.retiring.push(pending);
        if self.active == Some(id) {
            self.active = self
                .entries
                .get(index.min(self.entries.len().saturating_sub(1)))
                .map(|entry| entry.id);
        }
        self.confirming = None;
        self.focus(window, cx);
        cx.notify();
    }

    fn request_close(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing || self.confirming.is_some() {
            return;
        }
        let Some(entry) = self.entries.iter().find(|entry| entry.id == id) else {
            return;
        };
        if !entry.terminal.read(cx).is_running(cx) {
            self.remove(id, window, cx);
            return;
        }
        self.confirming = Some(id);
        let confirm = cx.entity().downgrade();
        let cancelled = confirm.clone();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let confirm = confirm.clone();
            let cancelled = cancelled.clone();
            let colors = gds::theme::Colors::current(cx);
            let mut background = colors.panel;
            background.a = 1.;
            dialog
                .confirm()
                .bg(background)
                .text_color(colors.panel_foreground)
                .title(crate::i18n::tr!("local-terminal-tab-close-title"))
                .description(crate::i18n::tr!("local-terminal-tab-close-message"))
                .footer(
                    gds::ui::DialogActions::new(
                        crate::i18n::tr!("local-terminal-tab-close"),
                        IconName::X,
                        crate::i18n::tr!("action-cancel"),
                    )
                    .danger(),
                )
                .on_ok(move |_, window, cx| {
                    if let Err(error) = confirm.update(cx, |panel, cx| panel.remove(id, window, cx))
                    {
                        tracing::debug!(%error, "terminal collection closed before confirmation");
                    }
                    true
                })
                .on_close(move |_, window, cx| {
                    if let Err(error) = cancelled.update(cx, |panel, cx| {
                        panel.confirming = None;
                        panel.focus(window, cx);
                        cx.notify();
                    }) {
                        tracing::debug!(%error, "terminal collection closed with dialog");
                    }
                })
        });
    }

    pub fn active_title(&self, cx: &App) -> SharedString {
        self.entries
            .iter()
            .find(|entry| Some(entry.id) == self.active)
            .map(|entry| Self::title(entry, cx))
            .unwrap_or_else(|| crate::i18n::tr!("local-terminal-title").into())
    }

    fn title(entry: &Entry, cx: &App) -> SharedString {
        if let Some(title) = entry
            .terminal
            .read(cx)
            .title(cx)
            .filter(|title| !title.trim().is_empty())
        {
            return title;
        }
        entry
            .terminal
            .read(cx)
            .cwd()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| crate::i18n::tr!("local-terminal-title").to_owned())
            .into()
    }

    fn tab(&self, entry: &Entry, collapsed: bool, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let id = entry.id;
        let title = Self::title(entry, cx);
        let cwd = entry.terminal.read(cx).cwd().display().to_string();
        let status = match entry.terminal.read(cx).status(cx) {
            Status::Starting => crate::i18n::tr!("local-terminal-starting").to_owned(),
            Status::Running => crate::i18n::tr!("local-terminal-running").to_owned(),
            Status::Stopping => crate::i18n::tr!("local-terminal-closing").to_owned(),
            Status::Closed => crate::i18n::tr!("local-terminal-closed").to_owned(),
            Status::Exited { code, .. } => {
                crate::i18n::tr!("local-terminal-exited", code = code.to_string())
            }
            Status::Failed(error) => error,
        };
        let origin = crate::i18n::tr!("local-terminal-started-in", path = cwd);
        let hint = format!("{title} · {origin} · {status}");
        let colors = gds::theme::Colors::current(cx);
        let selected = self.active == Some(id);
        let row = div()
            .id(("agents-terminal-tab", id))
            .test_support()
            .flex()
            .items_center()
            .min_w_0()
            .h(px(gds::ui::SIDEBAR_ITEM_HEIGHT))
            .rounded(px(6.))
            .px_2()
            .gap_1()
            .text_color(colors.sidebar_foreground)
            .hover(|row| row.bg(colors.hover()))
            .when(selected, |row| {
                row.bg(colors.selection)
                    .text_color(colors.selection_foreground)
            })
            .tooltip(move |window, cx| gds::ui::tooltip::view(hint.clone(), window, cx))
            .on_click(cx.listener(move |panel, _, window, cx| {
                panel.active = Some(id);
                panel.tabs_focus.focus(window, cx);
                cx.notify();
            }));
        if collapsed {
            row.justify_center()
                .child(Icon::new(IconName::Terminal).size(px(crate::ui::ICON)))
                .into_any_element()
        } else {
            row.child(div().flex_1().min_w_0().truncate().child(title))
                .child(
                    Button::icon_button(("agents-terminal-close", id))
                        .control_icon(IconName::X)
                        .selected(selected)
                        .tooltip(crate::i18n::tr!("local-terminal-tab-close"))
                        .on_click(cx.listener(move |panel, _, window, cx| {
                            cx.stop_propagation();
                            panel.request_close(id, window, cx);
                        })),
                )
                .into_any_element()
        }
    }
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let collapsed = self.sidebar_width <= gds::ui::COLLAPSED_SIDEBAR_WIDTH;
        let tabs = div()
            .id("agents-terminal-tabs")
            .test_support()
            .key_context(TABS_CONTEXT)
            .track_focus(&self.tabs_focus)
            .w(px(self.sidebar_width))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .p(px(gds::ui::SIDEBAR_PADDING))
            .gap_1()
            .text_color(gds::theme::Colors::current(cx).sidebar_foreground)
            .bg(gds::theme::sidebar_background(cx))
            .on_action(cx.listener(Self::new_terminal))
            .on_action(cx.listener(Self::close_terminal))
            .on_action(cx.listener(Self::next_terminal))
            .on_action(cx.listener(Self::previous_terminal))
            .on_action(cx.listener(Self::focus_terminal))
            .children(
                self.entries
                    .iter()
                    .map(|entry| self.tab(entry, collapsed, cx)),
            )
            .child(
                Button::new("agents-terminal-new")
                    .ghost()
                    .h(px(gds::ui::ICON_BUTTON_SIZE))
                    .w_full()
                    .when(!collapsed, |button| {
                        button.label(crate::i18n::tr!("local-terminal-new"))
                    })
                    .control_icon(IconName::Plus)
                    .disabled(self.cwd.is_none() || self.closing)
                    .tooltip_with_shortcut(
                        crate::i18n::tr!("local-terminal-new"),
                        gds::ui::Shortcut::new(Box::new(NewTerminal), Some(TABS_CONTEXT)),
                    )
                    .on_click(cx.listener(|panel, _, window, cx| {
                        panel.new_terminal(&NewTerminal, window, cx)
                    })),
            );
        let content = match self.active_terminal() {
            Some(terminal) => terminal.clone().into_any_element(),
            None => div()
                .size_full()
                .p_3()
                .bg(gds::theme::content_background(cx))
                .text_color(cx.theme().muted_foreground)
                .child(if self.cwd.is_some() {
                    crate::i18n::tr!("local-terminal-empty")
                } else {
                    crate::i18n::tr!("local-terminal-no-cwd")
                })
                .into_any_element(),
        };
        // A live Terminal owns its own content material; only the tab column and
        // empty state are painted here, never a background behind the PTY grid.
        div()
            .relative()
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .overflow_hidden()
            .child(tabs)
            .child(div().flex_1().min_h_0().min_w_0().h_full().child(content))
            .child(
                div()
                    .absolute()
                    .left(px(self.sidebar_width))
                    .top_0()
                    .bottom_0()
                    .w(px(1.))
                    .bg(cx.theme().border),
            )
    }
}
