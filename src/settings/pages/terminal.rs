use gds_uim::el;
use gpui_kit::*;

use crate::{
    i18n::text,
    settings::{View, controls::SettingNote},
};

impl View {
    pub(super) fn render_terminal(&self, cx: &App) -> Div {
        let terminal_note = SettingNote::new(text("settings-terminal-note", cx));
        let shell_note = SettingNote::new(text("settings-shell-note", cx));
        el! { .div { flex; flex_col; gap_4 } [terminal_note, shell_note] }
    }
}
