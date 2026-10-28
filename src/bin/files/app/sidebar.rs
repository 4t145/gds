//! Locations stay at the bottom; tabs and devices divide the remaining height.

use super::*;
use gds::ui::button::Button;
use gds_uim::el;
use gpui_kit::{
    assets::IconName,
    component::{ActiveTheme, Icon, Sizable, button::ButtonVariants},
    prelude::*,
};

pub(super) const MIN_TABS_HEIGHT: f32 = 94.;
pub(super) const MIN_DEVICES_HEIGHT: f32 = 108.;
const HEADER_HEIGHT: f32 = 34.;
const LOCATION_HEADER_HEIGHT: f32 = 28.;
const GROUP_GAP: f32 = 6.;
const ROW_GAP: f32 = 4.;
const DEFAULT_DEVICE_ROWS: usize = 4;

impl Browser {
    pub(super) fn on_sidebar_split(
        &mut self,
        _: Entity<ui::split::State>,
        event: &ui::split::Changed,
        cx: &mut Context<Self>,
    ) {
        self.preferences.files.sidebar.tabs_height = Some(event.0);
        self.save_sidebar(cx);
    }

    fn save_sidebar(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = settings::save(&self.preferences) {
            report_settings_failure("settings-save-failed", &error, cx);
        }
        cx.notify();
    }

    fn toggle_devices(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.regions.volumes.contains_focused(window, cx);
        self.preferences.files.sidebar.volumes_collapsed =
            !self.preferences.files.sidebar.volumes_collapsed || focused;
        if focused {
            self.focus.focus(window, cx);
        }
        self.save_sidebar(cx);
    }

    fn device_heading(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let collapsed = self.preferences.files.sidebar.volumes_collapsed
            && !self.regions.volumes.is_focused(window)
            && !self.regions.volumes.contains_focused(window, cx);
        let on_click = cx.listener(Self::toggle_devices);
        let on_manage = cx.listener(Self::click_manage_devices);
        let heading = el! {
            .div { flex; items_center; gap: GROUP_GAP.px; w_full; min_w_0 } [
                $(if collapsed { IconName::ChevronRight } else { IconName::ChevronDown }).Icon::new { size: 12.0.px },
                ($i18n::text("volumes", cx), cx).ui::sidebar_section_title,
            ]
        };
        el! {
            .div { flex; items_center; w_full; h: HEADER_HEIGHT.px; flex_shrink_0 } [
                "volumes".Button::new { ghost; small; flex_1; min_w_0; justify_start;
                    accessibility_label: $i18n::text("volumes", cx); on_click: on_click;
                } [heading],
                "manage-devices".ui::IconButton::new { icon: IconName::Settings2;
                    label: $i18n::text("volumes-manage", cx); on_click: on_manage;
                },
            ]
        }
        .into_any_element()
    }

    fn sidebar_tabs(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let cycle_targets = self.tab_cycle_targets();
        let tabs: Vec<_> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(position, slot)| self.render_tab(slot, position, cycle_targets, window, cx))
            .collect();
        let on_new_tab = cx.listener(Self::click_command(Command::NewTab));
        let content = el! {
            .div { flex; flex_col; size_full; min_h_0; px: ui::SIDEBAR_PADDING.px; pt: GROUP_GAP.px } [
                .div { flex; items_center; flex_shrink_0; h: HEADER_HEIGHT.px } [
                    @if (!self.sidebar_collapsed)
                        ($i18n::text("tabs", cx), cx).ui::sidebar_section_title { flex_1; px: GROUP_GAP.px },
                    @end,
                    "new-tab".ui::IconButton::new { icon: IconName::Plus; label: $i18n::text("new-tab", cx);
                        command: Command::NewTab; on_click: on_new_tab;
                    },
                ],
                .div { # "vertical-tabs"; flex; flex_col; gap: ROW_GAP.px; flex_1; min_h_0; overflow_y_scroll;
                    track_scroll: $&self.regions.tabs_scroll;
                } [..tabs],
            ]
        };
        ui::region::surface(
            "tabs-region",
            &self.regions.tabs,
            Command::FocusTabs,
            window,
            cx,
        )
        .flex()
        .flex_col()
        .size_full()
        .child(content)
        .into_any_element()
    }

    fn sidebar_locations(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let locations: Vec<_> = LocationTarget::ALL
            .into_iter()
            .map(|target| self.render_location(target, window, cx))
            .collect();
        let content = el! {
            .div { # "sidebar-locations"; flex; flex_col; flex_shrink_0; w_full;
                px: ui::SIDEBAR_PADDING.px; pb: ui::SIDEBAR_PADDING.px; gap: ROW_GAP.px;
            } [
                @if (!self.sidebar_collapsed)
                    ($i18n::text("locations", cx), cx).ui::sidebar_section_title {
                        flex; items_center; flex_shrink_0; h: LOCATION_HEADER_HEIGHT.px; px: GROUP_GAP.px;
                    },
                @end,
                ..locations,
            ]
        };
        ui::region::surface(
            "locations-region",
            &self.regions.locations,
            Command::FocusLocations,
            window,
            cx,
        )
        .w_full()
        .flex_shrink_0()
        .child(content)
        .into_any_element()
    }

    pub(super) fn render_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let tabs = self.sidebar_tabs(window, cx);
        let locations = self.sidebar_locations(window, cx);
        let show_volumes =
            self.preferences.files.sidebar.show_volumes || self.regions.reveal_volumes;
        let devices = if show_volumes {
            self.render_device_groups(true, window, cx)
        } else {
            Vec::new()
        };
        let device_row_count = devices.len();
        let mounted_count = self
            .storage
            .read(cx)
            .snapshot
            .volumes
            .iter()
            .filter(|volume| volume.visible && !volume.mount_points.is_empty())
            .count();
        let expanded = !self.preferences.files.sidebar.volumes_collapsed
            || self.regions.volumes.is_focused(window)
            || self.regions.volumes.contains_focused(window, cx);
        let heading = (!self.sidebar_collapsed).then(|| self.device_heading(window, cx));
        let workspace = if show_volumes && expanded {
            let device_area = el! {
                .div { flex; flex_col; size_full; min_h_0; px: ui::SIDEBAR_PADDING.px } [
                    ..heading,
                    .div { # "sidebar-devices"; flex; flex_col; flex_1; min_h_0; overflow_y_scroll;
                        gap: ROW_GAP.px; pb: ROW_GAP.px;
                        track_scroll: $&self.regions.volumes_scroll;
                    } [..devices,
                        @if (device_row_count == 0 && !self.sidebar_collapsed)
                            .div { px: GROUP_GAP.px; text_xs; text_color: $cx.theme().muted_foreground } [$i18n::text("device-none", cx)],
                        @end,
                    ],
                ]
            }
            .into_any_element();
            let device_area = ui::region::surface(
                "volumes-region",
                &self.regions.volumes,
                Command::FocusVolumes,
                window,
                cx,
            )
            .flex()
            .flex_col()
            .size_full()
            .child(device_area)
            .into_any_element();
            let default_bottom = device_row_count.min(DEFAULT_DEVICE_ROWS) as f32
                * (ui::SIDEBAR_ITEM_HEIGHT + ROW_GAP)
                + mounted_count.min(DEFAULT_DEVICE_ROWS) as f32
                    * if self.sidebar_collapsed {
                        ui::capacity::COMPACT_EXTRA_ROW_HEIGHT
                    } else {
                        ui::capacity::EXTRA_ROW_HEIGHT
                    }
                + if self.sidebar_collapsed {
                    0.
                } else {
                    HEADER_HEIGHT
                };
            self.sidebar_split
                .update(cx, |state, _| state.default_bottom(default_bottom));
            ui::split::VerticalSplit::new(
                self.sidebar_split.clone(),
                tabs,
                device_area,
                i18n::text("sidebar-resize", cx),
            )
            .into_any_element()
        } else {
            el! {
                .div { flex; flex_col; size_full; min_h_0 } [
                    .div { flex_1; min_h_0; overflow_hidden } [tabs],
                    @if (show_volumes)
                        .div { flex_shrink_0; px: ui::SIDEBAR_PADDING.px } [..heading],
                    @end,
                ]
            }
            .into_any_element()
        };
        el! {
            .div { # "file-sidebar-content"; flex; flex_col; size_full; min_h_0; overflow_hidden } [
                .div { flex_1; min_h_0; overflow_hidden } [workspace],
                locations,
            ]
        }
        .into_any_element()
    }
}
