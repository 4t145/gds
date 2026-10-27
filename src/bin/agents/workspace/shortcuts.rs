//! Thread shortcuts route through the same guarded operations as the visible controls.

use super::*;
use gpui_kit::{Action, AnyView, component::menu::PopupMenuItem};

actions!(
    bas,
    [
        RenameThread,
        ArchiveThread,
        ToggleThreadPin,
        ToggleThreadBookmark,
        DeleteThread,
        AddWorkspace,
        ToggleSession,
        OpenSessionManager,
        ScrollConversationToTop,
        PageConversationUp,
        PageConversationDown,
        PreviousConversationTurn,
        NextConversationTurn
    ]
);

pub(super) fn bind_keys(cx: &mut App) {
    for context in [
        "BasWorkspace",
        "BasWorkspace > Input",
        "BasWorkspace > BasComposer > Input",
    ] {
        cx.bind_keys([
            KeyBinding::new("f2", RenameThread, Some(context)),
            KeyBinding::new("ctrl-w", ArchiveThread, Some(context)),
            KeyBinding::new("ctrl-alt-p", ToggleThreadPin, Some(context)),
            KeyBinding::new("ctrl-d", ToggleThreadBookmark, Some(context)),
            KeyBinding::new("ctrl-alt-w", DeleteThread, Some(context)),
            KeyBinding::new("ctrl-alt-n", AddWorkspace, Some(context)),
            KeyBinding::new("ctrl-s", ToggleSession, Some(context)),
            KeyBinding::new("ctrl-m", OpenSessionManager, Some(context)),
            KeyBinding::new("ctrl-home", ScrollConversationToTop, Some(context)),
            KeyBinding::new("pageup", PageConversationUp, Some(context)),
            KeyBinding::new("pagedown", PageConversationDown, Some(context)),
            KeyBinding::new("ctrl-pageup", PreviousConversationTurn, Some(context)),
            KeyBinding::new("ctrl-pagedown", NextConversationTurn, Some(context)),
        ]);
    }
}

pub(super) fn menu_item(label: &'static str, action: Option<Box<dyn Action>>) -> PopupMenuItem {
    application::action_item(label, action, Some("BasWorkspace"))
}

pub(crate) fn settings_entries() -> Vec<(&'static str, gds::ui::Shortcut)> {
    let actions: Vec<(&str, Box<dyn Action>)> = vec![
        (crate::i18n::tr!("session-new"), Box::new(NewSession)),
        (crate::i18n::tr!("session-rename"), Box::new(RenameThread)),
        (crate::i18n::tr!("session-archive"), Box::new(ArchiveThread)),
        (crate::i18n::tr!("session-pin"), Box::new(ToggleThreadPin)),
        (
            crate::i18n::tr!("session-bookmark"),
            Box::new(ToggleThreadBookmark),
        ),
        (crate::i18n::tr!("session-delete"), Box::new(DeleteThread)),
        (crate::i18n::tr!("workspace-add"), Box::new(AddWorkspace)),
        (
            crate::i18n::tr!("agent-session-close"),
            Box::new(ToggleSession),
        ),
        (
            crate::i18n::tr!("agent-history-title"),
            Box::new(OpenSessionManager),
        ),
        (crate::i18n::tr!("shortcuts-search"), Box::new(FocusSearch)),
        (
            crate::i18n::tr!("shortcuts-composer"),
            Box::new(FocusComposer),
        ),
        (crate::i18n::tr!("sidebar-toggle"), Box::new(ToggleSidebar)),
        (
            crate::i18n::tr!("conversation-toggle"),
            Box::new(ToggleOutput),
        ),
        (
            crate::i18n::tr!("shortcuts-window-mode"),
            Box::new(ToggleMode),
        ),
        (
            crate::i18n::tr!("shortcuts-next-thread"),
            Box::new(NextThread),
        ),
        (
            crate::i18n::tr!("shortcuts-previous-thread"),
            Box::new(PreviousThread),
        ),
        (crate::i18n::tr!("shortcuts-refresh"), Box::new(Refresh)),
        (
            crate::i18n::tr!("conversation-top"),
            Box::new(ScrollConversationToTop),
        ),
        (
            crate::i18n::tr!("conversation-latest"),
            Box::new(ScrollConversationToEnd),
        ),
        (
            crate::i18n::tr!("shortcuts-page-up"),
            Box::new(PageConversationUp),
        ),
        (
            crate::i18n::tr!("shortcuts-page-down"),
            Box::new(PageConversationDown),
        ),
        (
            crate::i18n::tr!("turn-marker-previous"),
            Box::new(PreviousConversationTurn),
        ),
        (
            crate::i18n::tr!("turn-marker-next"),
            Box::new(NextConversationTurn),
        ),
    ];
    actions
        .into_iter()
        .map(|(label, action)| (label, gds::ui::Shortcut::new(action, Some("BasWorkspace"))))
        .collect()
}

pub(super) fn hint(label: &str, action: &dyn Action, window: &Window) -> String {
    gds::ui::Shortcut::new(action.boxed_clone(), Some("BasWorkspace")).hint(label, window)
}

pub(super) fn tooltip(
    label: &'static str,
    action: impl Action,
) -> impl Fn(&mut Window, &mut App) -> AnyView {
    gds::ui::shortcut_tooltip(
        label,
        gds::ui::Shortcut::new(Box::new(action), Some("BasWorkspace")),
    )
}

impl Workspace {
    pub(super) fn on_toggle_session(
        &mut self,
        _: &ToggleSession,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(meta) = self.current().filter(|meta| !meta.archived) else {
            return;
        };
        let id = meta.id;
        if meta.status == SessionStatus::Stopped {
            self.request_restart_agent_session(id, cx);
        } else {
            self.request_close_agent_session(id, cx);
        }
    }

    pub(super) fn on_open_session_manager(
        &mut self,
        _: &OpenSessionManager,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agent = self
            .current()
            .map(|meta| meta.agent.to_string())
            .unwrap_or_else(|| self.selected.agent.clone());
        self.open_agent_history(agent, window, cx);
    }

    pub(super) fn on_conversation_top(
        &mut self,
        _: &ScrollConversationToTop,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.scroll_conversation_to_top(cx);
    }

    pub(super) fn on_conversation_page_up(
        &mut self,
        _: &PageConversationUp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.page_conversation(true, cx);
    }

    pub(super) fn on_conversation_page_down(
        &mut self,
        _: &PageConversationDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.page_conversation(false, cx);
    }

    pub(super) fn on_conversation_previous_turn(
        &mut self,
        _: &PreviousConversationTurn,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_conversation_turn(true, cx);
    }

    pub(super) fn on_conversation_next_turn(
        &mut self,
        _: &NextConversationTurn,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_conversation_turn(false, cx);
    }

    pub(super) fn on_rename_thread(
        &mut self,
        _: &RenameThread,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_menu(window, cx);
        if self.window.compact {
            self.toggle_mode(window, cx);
        }
        self.begin_rename(window, cx);
    }

    pub(super) fn on_archive_thread(
        &mut self,
        _: &ArchiveThread,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(meta) = self.current() {
            self.set_archived(meta.id, !meta.archived, cx);
        }
    }

    pub(super) fn on_toggle_thread_pin(
        &mut self,
        _: &ToggleThreadPin,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.selected.id
            && !self.sessions.pending.deleting.contains(&id)
            && self.sessions.entries.iter().any(|session| session.id == id)
        {
            self.toggle_pinned(id, cx);
        }
    }

    pub(super) fn on_toggle_thread_bookmark(
        &mut self,
        _: &ToggleThreadBookmark,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.selected.id {
            self.toggle_bookmark(id, cx);
        }
    }

    pub(super) fn on_delete_thread(
        &mut self,
        _: &DeleteThread,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.selected.id {
            self.request_delete(id, window, cx);
        }
    }

    pub(super) fn on_add_workspace(
        &mut self,
        _: &AddWorkspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.choose_local_workspace(window, cx);
    }
}
