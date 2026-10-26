use gds_uim::el;
use gpui_kit::*;

use crate::{
    i18n::text,
    settings::{
        View,
        controls::{Field, SettingRow},
    },
};

impl View {
    pub(super) fn render_general(&mut self, cx: &mut Context<Self>) -> Div {
        let select = self.language.render(
            Field::Language,
            text("language-system", cx),
            text("language-search", cx),
            cx,
        );
        let language_row = SettingRow::editable(Field::Language, self, select, cx);
        let time_format = self.time_format.render(
            Field::TimeFormat,
            text("time-format-relative", cx),
            text("language-search", cx),
            cx,
        );
        let time_format_row = SettingRow::editable(Field::TimeFormat, self, time_format, cx);
        el! { .div [language_row, time_format_row] }
    }
}
