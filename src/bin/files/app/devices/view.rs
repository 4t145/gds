use super::*;
use gds::storage::{DeviceKind, Volume};
use gds::ui::{IconButton, NavItem, ShortcutBadge, capacity::CapacityBar};
use gpui_kit::component::{
    Sizable,
    button::{Button, ButtonVariants},
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
    spinner::Spinner,
};

const GAP: f32 = 8.;
const PAGE_PADDING: f32 = 24.;
const MENU_WIDTH: f32 = 220.;

fn menu_row(
    label: SharedString,
    command: Option<Command>,
) -> impl Fn(&mut Window, &mut App) -> Div {
    move |_, _| {
        el! {
            .div { flex; items_center; w_full; min_w: MENU_WIDTH.px; gap: PAGE_PADDING.px } [
                .div { flex_1; min_w_0; truncate } [$(label.clone())],
                @if (let Some(command) = command) command.ShortcutBadge::new, @end,
            ]
        }
    }
}

pub(super) fn device_icon(volume: &Volume, snapshot: &Snapshot) -> IconName {
    if volume.locked {
        return IconName::Lock;
    }
    match volume
        .drive
        .as_deref()
        .and_then(|id| snapshot.drive(id))
        .map(|drive| drive.kind)
    {
        Some(DeviceKind::Usb) => IconName::Usb,
        Some(DeviceKind::Optical) => IconName::Disc,
        _ => IconName::HardDrive,
    }
}

impl Browser {
    pub(in super::super) fn render_device_groups(
        &self,
        _: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let snapshot = self.storage.read(cx).snapshot.clone();
        snapshot
            .volumes
            .iter()
            .filter(|volume| volume.visible)
            .map(|volume| self.render_sidebar_volume(volume, &snapshot, window, cx))
            .collect()
    }

    fn volume_details(&self, volume: &Volume, cx: &App) -> SharedString {
        let mut lines = vec![volume.name.clone(), volume.filesystem.clone()];
        if volume.system_mount {
            lines.push(i18n::text("device-system-volume", cx).to_string());
        }
        if let Some(root) = self.default_volume_root(volume) {
            lines.push(format!(
                "{}: {}",
                i18n::text("volume-default-mount", cx),
                root.display()
            ));
        }
        if let Some(storage::CapacityState::Ready(capacity)) =
            self.storage.read(cx).capacity(&volume.id)
        {
            lines.push(Self::capacity_label(*capacity, cx).to_string());
        }
        lines.join("\n").into()
    }

    pub(super) fn capacity_label(capacity: storage::Capacity, cx: &App) -> SharedString {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set(
            "available",
            super::super::table::EntryPresentation::format_size(capacity.available),
        );
        args.set(
            "total",
            super::super::table::EntryPresentation::format_size(capacity.total),
        );
        i18n::format("device-capacity", Some(&args), cx)
    }

    pub(super) fn capacity_bar(
        id: String,
        state: Option<&storage::CapacityState>,
        compact: bool,
        cx: &App,
    ) -> CapacityBar {
        match state {
            Some(storage::CapacityState::Ready(capacity)) => {
                CapacityBar::new(id, Self::capacity_label(*capacity, cx))
                    .value(capacity.unavailable_fraction())
            }
            Some(storage::CapacityState::Unavailable(_)) => {
                CapacityBar::new(id, i18n::text("device-capacity-unavailable", cx)).unavailable()
            }
            _ => CapacityBar::new(id, i18n::text("device-capacity-loading", cx)),
        }
        .compact(compact)
    }

    fn render_sidebar_volume(
        &self,
        volume: &Volume,
        snapshot: &Snapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let compact = self.sidebar_collapsed;
        let id = format!("sidebar-volume-{}", volume.object);
        let selected = self.collection.is_none()
            && self
                .volume_pages
                .active(self.active_id)
                .is_some_and(|page| page.volume == volume.id);
        let busy = self.device_is_busy(&volume.id, cx);
        let details = if volume.mount_points.is_empty() {
            (!compact).then(|| el! {
                .div { px: GAP.px; pb: GAP.px; text_xs; text_color: $cx.theme().muted_foreground } [$i18n::text("device-not-mounted", cx)]
            }.into_any_element())
        } else {
            let bar = Self::capacity_bar(
                format!("{id}-capacity"),
                self.storage.read(cx).capacity(&volume.id),
                compact,
                cx,
            );
            Some(el! { .div { px: GAP.px; w_full; min_w_0 } [bar] }.into_any_element())
        };
        let nav = NavItem::new(
            id.clone(),
            device_icon(volume, snapshot),
            volume.name.clone(),
        )
        .tooltip_text(self.volume_details(volume, cx))
        .tooltip_enabled(
            self.keyboard_menu.is_none()
                && self
                    .volume_menu_focus
                    .as_ref()
                    .is_none_or(|focus| !focus.contains_focused(window, cx)),
        )
        .compact(compact)
        .selected(selected)
        .on_click(cx.listener(Self::click_volume(volume.id.clone())))
        .when_some(details, |nav, details| nav.details(details))
        .trailing(
            busy.then(|| Spinner::new().small().into_any_element())
                .into_iter()
                .collect(),
        );
        el! { .div { # id; w_full; flex_shrink_0 } [nav] }
            .context_menu(Self::volume_menu(
                cx.entity().downgrade(),
                volume.id.clone(),
            ))
            .into_any_element()
    }

    pub(super) fn volume_menu(
        browser: WeakEntity<Self>,
        id: VolumeId,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu {
        move |menu, window, cx| {
            let focus = menu.focus_handle(cx);
            let available = browser
                .update(cx, |view, cx| {
                    view.volume_menu_focus = Some(focus);
                    cx.notify();
                    if view.collection == Some(Collection::Volumes) {
                        view.select_volume(id.clone(), window, cx);
                    }
                    view.volume_can_open(&id, cx)
                })
                .unwrap_or(false);
            if available {
                Self::prepare_volume_open(&menu, window, cx);
            }
            Self::volume_menu_contents(browser.clone(), &id, menu, cx)
        }
    }

    pub(in super::super) fn volume_menu_contents(
        browser: WeakEntity<Self>,
        id: &VolumeId,
        menu: PopupMenu,
        cx: &App,
    ) -> PopupMenu {
        let Some(view) = browser.upgrade() else {
            return menu;
        };
        view.read(cx).build_volume_menu(browser, id, menu, cx)
    }

    pub(in super::super) fn build_volume_menu(
        &self,
        browser: WeakEntity<Self>,
        id: &VolumeId,
        mut menu: PopupMenu,
        cx: &App,
    ) -> PopupMenu {
        let Some(volume) = self.storage.read(cx).snapshot.volume(id) else {
            return menu;
        };
        let target_matches = self.volume_action_target(cx).as_ref() == Some(id);
        let available = self.volume_can_open(id, cx);
        let target = browser.clone();
        let open_id = id.clone();
        let render = menu_row(i18n::text("open", cx), Some(Command::Open));
        menu = menu.item(
            PopupMenuItem::element(render)
                .disabled(!available)
                .on_click(move |_, window, cx| {
                    let _ = target.update(cx, |view, cx| {
                        if view.volume_can_open(&open_id, cx) {
                            view.open_volume(open_id.clone(), window, cx);
                        }
                    });
                }),
        );
        let command = if volume.mount_points.is_empty() {
            Command::MountVolume
        } else {
            Command::UnmountVolume
        };
        let enabled = self.volume_command_enabled(id, command, cx);
        let render = menu_row(
            i18n::text(command.label_id(), cx),
            target_matches.then_some(command),
        );
        let id = id.clone();
        menu.item(PopupMenuItem::element(render).disabled(!enabled).on_click(
            move |_, window, cx| {
                let _ = browser.update(cx, |view, cx| {
                    view.execute_volume_command(id.clone(), command, window, cx)
                });
            },
        ))
    }

    pub(super) fn volume_menu_button(
        browser: WeakEntity<Self>,
        id: VolumeId,
        element_id: String,
        cx: &App,
    ) -> AnyElement {
        Button::new(element_id)
            .ghost()
            .small()
            .size(px(ui::ICON_BUTTON_SIZE))
            .icon(IconName::Ellipsis)
            .accessibility_label(i18n::text("volume-actions", cx))
            .tooltip(i18n::text("volume-actions", cx))
            .dropdown_menu(Self::volume_menu(browser, id))
            .into_any_element()
    }

    pub(super) fn prepare_volume_open(menu: &PopupMenu, window: &mut Window, cx: &App) {
        let focus = menu.focus_handle(cx);
        window.on_next_frame(move |window, _| {
            // The popup's dispatch node exists after its first painted frame.
            window.on_next_frame(move |window, cx| {
                if focus.is_focused(window) {
                    window.dispatch_action(Box::new(gpui_kit::base::actions::SelectDown), cx);
                }
            });
        });
    }

    pub(super) fn default_mount_menu(
        browser: WeakEntity<Self>,
        id: VolumeId,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu {
        move |mut menu, window, cx| {
            let _ = browser.update(cx, |view, cx| {
                if view.collection == Some(Collection::Volumes) {
                    view.select_volume(id.clone(), window, cx);
                }
            });
            let Some(view) = browser.upgrade() else {
                return menu;
            };
            let view = view.read(cx);
            let Some(volume) = view.storage.read(cx).snapshot.volume(&id).cloned() else {
                return menu;
            };
            let selected = view.default_volume_root(&volume);
            for root in volume.mount_points {
                let target = browser.clone();
                let id = id.clone();
                let checked = selected.as_ref() == Some(&root);
                menu = menu.item(
                    PopupMenuItem::new(root.display().to_string())
                        .checked(checked)
                        .on_click(move |_, _, cx| {
                            let _ = target.update(cx, |view, cx| {
                                view.set_default_volume_root(&id, root.clone(), cx)
                            });
                        }),
                );
            }
            menu
        }
    }

    pub(in super::super) fn render_devices_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let service = self.storage.read(cx);
        let rows: Vec<_> = service
            .snapshot
            .volumes
            .iter()
            .filter(|volume| volume.visible)
            .map(|volume| table::Row {
                volume: volume.clone(),
                root: self.default_volume_root(volume),
                capacity: service.capacity(&volume.id).cloned(),
                icon: device_icon(volume, &service.snapshot),
            })
            .collect();
        let error = service.error.clone();
        let discovering = service.discovering;
        let width = f32::from(window.bounds().size.width) - self.width() - PAGE_PADDING * 2.;
        let row_count = rows.len();
        table::VolumeTable::sync(
            &self.volume_table,
            rows,
            self.selected_volume.clone(),
            width,
            cx,
        );
        let selected = self.volume_action_target(cx);
        let can_mount = selected
            .as_ref()
            .is_some_and(|id| self.volume_command_enabled(id, Command::MountVolume, cx));
        let can_unmount = selected
            .as_ref()
            .is_some_and(|id| self.volume_command_enabled(id, Command::UnmountVolume, cx));
        let on_mount = cx.listener(Self::click_command(Command::MountVolume));
        let on_unmount = cx.listener(Self::click_command(Command::UnmountVolume));
        let on_refresh = cx.listener(Self::click_command(Command::Refresh));
        let table = el! {
            ($&self.volume_table).gpui_kit::component::table::DataTable::new {
                bordered: false; with_size: $(gpui_kit::component::Size::Size(px(ui::DataListStyle::ROW_HEIGHT)));
                header_row_height: ui::DataListStyle::HEADER_HEIGHT.px;
                into_any_element;
            }
        };
        let scroll = self
            .volume_table
            .read(cx)
            .vertical_scroll_handle
            .0
            .borrow()
            .base_handle
            .clone();
        let list = ui::ListSurface::new("volume-selection-surface", table, row_count, scroll)
            .on_blank_press(cx.listener(Self::on_list_background_press));
        el! {
            .div { # "volumes-page"; flex; flex_col; size_full; min_h_0; min_w_0; px: PAGE_PADDING.px } [
                .div { flex; items_center; flex_shrink_0; gap: GAP.px; py: GAP.px } [
                    .div { flex_1; text_lg; font_weight: FontWeight::SEMIBOLD } [$i18n::text("volumes", cx)],
                    "mount-volume".IconButton::new { icon: IconName::Plug; label: $i18n::text("device-mount", cx); command: Command::MountVolume; disabled: $(!can_mount); on_click: on_mount },
                    "unmount-volume".IconButton::new { icon: IconName::Unplug; label: $i18n::text("device-unmount", cx); command: Command::UnmountVolume; disabled: $(!can_unmount); on_click: on_unmount },
                    "refresh-volume-capacity".IconButton::new { icon: IconName::RefreshCw; label: $i18n::text("refresh", cx); command: Command::Refresh; on_click: on_refresh },
                ],
                @if (let Some(error) = error)
                    .div { text_xs; text_color: $cx.theme().danger; whitespace_normal } [error],
                @else if (discovering)
                    .div { text_xs; text_color: $cx.theme().muted_foreground } [$i18n::text("device-discovering", cx)],
                @end,
                list,
            ]
        }.into_any_element()
    }

    pub(in super::super) fn render_device_unavailable(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(page) = self.volume_pages.active(self.active_id) {
            let volume = self.storage.read(cx).snapshot.volume(&page.volume);
            let name = volume
                .map(|volume| volume.name.clone())
                .unwrap_or_else(|| page.slot.title());
            let message = if volume.is_none() {
                "device-disconnected"
            } else if volume.is_some_and(|volume| volume.mount_points.is_empty()) {
                "device-not-mounted"
            } else {
                "volume-default-unavailable"
            };
            return el! {
                .div { size_full; flex; flex_col; min_h_0 } [
                    .div { flex; items_center; px: PAGE_PADDING.px; py: GAP.px } [
                        .div { flex_1; text_lg } [name],
                    ],
                    .div { flex_1; flex; flex_col; justify_center; items_center; gap: GAP.px } [
                        IconName::HardDrive.Icon::new,
                        .div { text_sm; text_color: $cx.theme().muted_foreground } [$i18n::text(message, cx)],
                    ],
                ]
            }.into_any_element();
        }
        let location = self.active().and_then(|slot| slot.device.as_ref());
        let unmounted =
            location.is_some_and(|location| location.availability == Availability::Unmounted);
        let retry = cx.listener(Self::click_reconnect);
        el! {
            .div { size_full; flex; flex_col; justify_center; items_center; p: PAGE_PADDING.px; gap: GAP.px } [
                IconName::HardDrive.Icon::new,
                .div { text_lg } [$i18n::text(if unmounted { "device-not-mounted" } else { "device-disconnected" }, cx)],
                .div { text_sm; text_color: $cx.theme().muted_foreground; whitespace_normal } [$(self.current_path().display().to_string())],
                "device-reconnect".Button::new { ghost; label: $i18n::text("device-reconnect", cx); on_click: retry },
            ]
        }.into_any_element()
    }
}
