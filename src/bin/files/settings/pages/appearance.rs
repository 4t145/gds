use gds_uim::el;
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

use crate::settings::{
    View,
    controls::{FONT_SIZE_RANGE, Field, PERCENTAGE_RANGE, SettingNote, SettingRow},
};
use gds::i18n::text;

impl View {
    pub(super) fn render_appearance(&mut self, cx: &mut Context<Self>) -> Div {
        let source = self.source.render(
            Field::Source,
            text("theme-custom", cx),
            text("language-search", cx),
            cx,
        );
        let mode = self.mode.render(
            Field::Mode,
            text("theme-mode", cx),
            text("language-search", cx),
            cx,
        );
        let source_row = SettingRow::editable(Field::Source, self, source, cx);
        let mode_row = SettingRow::editable(Field::Mode, self, mode, cx);
        let content_opacity = self.percentage_row(Field::ContentOpacity, cx);
        let sidebar_opacity = self.percentage_row(Field::SidebarOpacity, cx);
        let family = self.font_family.render(
            Field::FontFamily,
            text("font-family-system", cx),
            text("font-family-search", cx),
            cx,
        );
        let family_row = SettingRow::editable(Field::FontFamily, self, family, cx);
        let size_row = self.font_size_row(cx);
        let sample_text = text("font-sample", cx);
        let sample = el! {
            .div {
                py_4; text_lg; truncate;
                text_color: $(cx.theme().muted_foreground);
                border_b_1; border_color: $(cx.theme().border);
            } [sample_text]
        };
        let font_note = SettingNote::new(text("font-fallback-note", cx));
        let blur_note = SettingNote::new(text("blur-status", cx));
        el! {
            .div { flex; flex_col } [
                source_row,
                mode_row,
                content_opacity,
                sidebar_opacity,
                family_row,
                size_row,
                sample,
                .div { pt_4 } [font_note],
                .div { pt_2 } [blur_note],
            ]
        }
    }

    fn percentage_row(&self, field: Field, cx: &mut Context<Self>) -> SettingRow {
        let value = field
            .number(&self.preferences)
            .expect("opacity rows use numeric fields");
        let value_label = format!("{:.0}%", PERCENTAGE_RANGE.quantize(value)).into();
        let control = self.number_control(field).render(field, value_label, cx);
        SettingRow::editable(field, self, control, cx)
    }

    fn font_size_row(&self, cx: &mut Context<Self>) -> SettingRow {
        let value = Field::FontSize
            .number(&self.preferences)
            .expect("the font size field owns a numeric control");
        let value_label = format!("{:.0} px", FONT_SIZE_RANGE.quantize(value)).into();
        let control = self
            .number_control(Field::FontSize)
            .render(Field::FontSize, value_label, cx);
        SettingRow::editable(Field::FontSize, self, control, cx)
    }
}
