//! Application labels and help use the suite's settings row geometry.

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{AnyElement, App, IntoElement, RenderOnce, TestSupportExt, Window, div, prelude::*};

pub(super) const CONTROLS_WIDTH: f32 = gds::ui::settings::CONTROL_WIDTH;

#[derive(IntoElement)]
pub(super) struct Row {
    pub id: String,
    pub label: &'static str,
    pub help: &'static str,
    pub control: AnyElement,
    pub reset: Option<AnyElement>,
}

impl RenderOnce for Row {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let title = div()
            .id(format!("{}-label", self.id))
            .test_support()
            .tooltip(move |window, cx| Tooltip::new(self.help).build(window, cx))
            .child(self.label);
        let row = gds::ui::settings::SettingRow::new(title, self.control).reset(self.reset);
        div()
            .id(format!("{}-row", self.id))
            .test_support()
            .child(row)
    }
}
