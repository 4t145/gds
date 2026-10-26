use gds_uim::el;
use gpui_kit::{component::switch::Switch, *};

use crate::{
    i18n::text,
    settings::{
        View,
        controls::{Field, SettingNote, SettingRow},
    },
};

impl View {
    pub(super) fn render_files(&mut self, cx: &mut Context<Self>) -> Div {
        let on_hidden_changed = cx.listener(Self::on_hidden_changed);
        let toggle = el! {
            "settings-show-hidden".Switch::new {
                accessibility_label: $(text("show-hidden", cx));
                checked: $self.preferences.files.show_hidden;
                on_change: on_hidden_changed;
            }
        };
        let row = SettingRow::editable(Field::ShowHidden, self, toggle, cx);
        let note = SettingNote::new(text("settings-files-note", cx));
        el! { .div [note, row] }
    }
}
