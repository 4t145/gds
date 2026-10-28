//! Local shells belong to a backend/work-directory pair, never to a conversation.

use super::*;
use std::path::Path;

mod panel;
use panel::Panel;

pub(crate) const TABS_CONTEXT: &str = "AgentsTerminalTabs";

actions!(
    agents_terminal,
    [
        NewTerminal,
        CloseTerminal,
        NextTerminal,
        PreviousTerminal,
        FocusTerminal,
        FocusTerminalComposer
    ]
);

pub(super) fn bind_keys(cx: &mut App) {
    use gds::terminal::{
        Copy, INPUT_CONTEXT, Paste, ScrollBottom, ScrollPageDown, ScrollPageUp, ScrollTop,
    };
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-c", Copy, Some(INPUT_CONTEXT)),
        KeyBinding::new("ctrl-shift-v", Paste, Some(INPUT_CONTEXT)),
        KeyBinding::new("shift-pageup", ScrollPageUp, Some(INPUT_CONTEXT)),
        KeyBinding::new("shift-pagedown", ScrollPageDown, Some(INPUT_CONTEXT)),
        KeyBinding::new("shift-home", ScrollTop, Some(INPUT_CONTEXT)),
        KeyBinding::new("shift-end", ScrollBottom, Some(INPUT_CONTEXT)),
        KeyBinding::new("f6", FocusTerminalComposer, Some(INPUT_CONTEXT)),
        KeyBinding::new("f6", FocusTerminalComposer, Some(TABS_CONTEXT)),
        KeyBinding::new("ctrl-n", NewTerminal, Some(TABS_CONTEXT)),
        KeyBinding::new("ctrl-w", CloseTerminal, Some(TABS_CONTEXT)),
        KeyBinding::new("down", NextTerminal, Some(TABS_CONTEXT)),
        KeyBinding::new("up", PreviousTerminal, Some(TABS_CONTEXT)),
        KeyBinding::new("enter", FocusTerminal, Some(TABS_CONTEXT)),
    ]);
}

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Group {
    server: Target,
    cwd: PathBuf,
}

impl Group {
    fn new(server: &Target, cwd: Option<&Path>) -> Option<Self> {
        if !local_ipc(&server.target) {
            return None;
        }
        let cwd = cwd.filter(|path| path.is_absolute())?;
        Some(Self {
            server: server.clone(),
            cwd: navigation::groups::normalize(cwd),
        })
    }
}

fn local_ipc(target: &bas::service::Target) -> bool {
    use bas::service::Target;
    match target {
        Target::Local => target.resolved().is_ok_and(|target| match target {
            Target::Unix(_) => cfg!(unix),
            Target::Pipe(name) => cfg!(windows) && local_pipe(&name),
            _ => false,
        }),
        Target::Unix(_) => cfg!(unix),
        Target::Pipe(name) => cfg!(windows) && local_pipe(name),
        // Loopback TCP can be forwarded to another machine. Never infer locality.
        Target::Tcp(_) | Target::Mtls(_) => false,
    }
}

fn local_pipe(name: &str) -> bool {
    name.starts_with(r"\\.\pipe\") || (!name.contains('\\') && !name.contains('/'))
}

pub(super) struct Terminals {
    panels: HashMap<Group, Entity<Panel>>,
    pub visible: bool,
    pub split: Entity<gds::ui::split::State>,
}

impl Terminals {
    pub fn new(cx: &mut App) -> Self {
        Self {
            panels: HashMap::new(),
            visible: false,
            split: cx.new(|cx| {
                let mut split = gds::ui::split::State::new(None, 160., 140., cx);
                split.default_bottom(260.);
                split
            }),
        }
    }
}

impl Workspace {
    fn terminal_group(&self) -> Option<Group> {
        let cwd = if self.selected.id.is_some() {
            // A session without cwd must not inherit the previously selected directory.
            self.current().and_then(|session| session.cwd.as_deref())
        } else {
            self.navigation.selected_directory.as_deref()
        };
        Group::new(&self.backend.server, cwd)
    }

    pub(super) fn terminal_panel(&self) -> Option<Entity<Panel>> {
        self.terminals.panels.get(&self.terminal_group()?).cloned()
    }

    pub(super) fn sync_terminal_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.terminals.visible || self.window.compact {
            return;
        }
        let Some(group) = self.terminal_group() else {
            return;
        };
        if !self.terminals.panels.contains_key(&group) {
            let preferences = crate::configuration::terminal::current(cx);
            let panel = cx.new(|cx| {
                let mut panel = Panel::new(cx);
                panel.set_preferences(preferences, cx);
                panel.set_cwd(Some(group.cwd.clone()), cx);
                panel
            });
            self.terminals.panels.insert(group, panel);
        }
        if let Some(panel) = self.terminal_panel()
            && panel.read(cx).needs_initial_reveal()
        {
            // This sync also runs during rendering. Start only after the frame,
            // provided this owner is still visible and selected.
            let owner = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    if this.terminals.visible
                        && this.window.presented
                        && !this.window.compact
                        && this.terminal_panel().is_some_and(|active| active == panel)
                        && panel.read(cx).needs_initial_reveal()
                    {
                        panel.update(cx, |panel, cx| panel.reveal(window, cx));
                    }
                });
            });
        }
    }

    pub(crate) fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.window.compact {
            self.toggle_mode(window, cx);
            self.terminals.visible = true;
        } else {
            self.terminals.visible = !self.terminals.visible;
        }
        self.sync_terminal_panel(window, cx);
        if self.terminals.visible {
            if let Some(panel) = self.terminal_panel() {
                panel.update(cx, |panel, cx| panel.reveal(window, cx));
            }
        } else {
            self.focus_input(window, cx);
        }
        cx.notify();
    }

    pub(super) fn on_terminal_focus_composer(
        &mut self,
        _: &FocusTerminalComposer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.terminal_focused(window, cx) {
            self.focus_input(window, cx);
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }

    pub(super) fn terminal_focused(&self, window: &Window, cx: &App) -> bool {
        self.terminals
            .panels
            .values()
            .any(|panel| panel.read(cx).contains_focus(window, cx))
    }

    pub(crate) fn has_running_terminals(&self, cx: &App) -> bool {
        self.terminals
            .panels
            .values()
            .any(|panel| panel.read(cx).has_running_sessions(cx))
    }

    pub(crate) fn shutdown_terminals(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Vec<gds::terminal::Shutdown> {
        self.terminals
            .panels
            .values()
            .map(|panel| panel.update(cx, |panel, cx| panel.shutdown(cx)))
            .collect()
    }

    pub(super) fn update_terminal_preferences(&mut self, cx: &mut Context<Self>) {
        let preferences = crate::configuration::terminal::current(cx);
        for panel in self.terminals.panels.values() {
            panel.update(cx, |panel, cx| {
                panel.set_preferences(preferences.clone(), cx)
            });
        }
        cx.notify();
    }

    pub(super) fn terminal_toggle_button(&self, cx: &mut Context<Self>) -> Button {
        layout::icon_button("toggle-local-terminal", IconName::Terminal)
            .tooltip_with_shortcut(
                crate::i18n::tr!("local-terminal-toggle"),
                gds::ui::Shortcut::new(Box::new(ToggleTerminal), Some("AgentsHost")),
            )
            .on_click(cx.listener(|this, _, window, cx| this.toggle_terminal(window, cx)))
    }

    pub(super) fn render_terminal_dock(
        &self,
        content: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.terminals.visible {
            return content;
        }
        let sidebar_width = if self.window.sidebar_visible {
            SIDEBAR_WIDTH
        } else {
            gds::ui::COLLAPSED_SIDEBAR_WIDTH
        };
        let frame = gds::ui::Frame::new(sidebar_width);
        let collapsed = !self.window.sidebar_visible;
        let heading = div()
            .id("agents-terminal-heading")
            .flex()
            .items_center()
            .h_full()
            .min_w_0()
            .px_2()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .when(collapsed, |heading| {
                heading
                    .justify_center()
                    .child(Icon::new(IconName::Terminal).size(px(crate::ui::ICON)))
                    .tooltip(|window, cx| {
                        gds::ui::tooltip::view(crate::i18n::tr!("local-terminal-title"), window, cx)
                    })
            })
            .when(!collapsed, |heading| {
                heading.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .child(crate::i18n::tr!("local-terminal-title")),
                )
            });
        let controls = div()
            .flex()
            .items_center()
            .justify_end()
            .h_full()
            .px_2()
            .child(
                layout::icon_button("hide-local-terminal", IconName::X)
                    .tooltip(crate::i18n::tr!("local-terminal-toggle"))
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_terminal(window, cx))),
            );
        let header = frame
            .workspace(heading.into_any_element(), controls.into_any_element(), cx)
            .flex_none()
            .h(px(crate::ui::CONTROL));
        let body = match self.terminal_panel() {
            Some(panel) => {
                panel.update(cx, |panel, cx| panel.set_sidebar_width(sidebar_width, cx));
                panel.into_any_element()
            }
            None => frame
                .workspace(
                    div().into_any_element(),
                    div()
                        .p_3()
                        .text_color(cx.theme().muted_foreground)
                        .child(if local_ipc(&self.backend.server.target) {
                            crate::i18n::tr!("local-terminal-no-cwd")
                        } else {
                            crate::i18n::tr!("local-terminal-remote")
                        })
                        .into_any_element(),
                    cx,
                )
                .h_full()
                .into_any_element(),
        };
        let dock = div()
            .id("local-terminal-dock")
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
                    .child(body),
            );
        let divider = frame
            .workspace(div().into_any_element(), div().into_any_element(), cx)
            .flex_none()
            .size_full();
        gds::ui::split::VerticalSplit::new(
            self.terminals.split.clone(),
            content,
            dock.into_any_element(),
            crate::i18n::tr!("local-terminal-resize"),
        )
        .divider_background(divider)
        .into_any_element()
    }
}
