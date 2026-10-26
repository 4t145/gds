use super::*;

use gds::ui::{DataListStyle, Frame, IconButton, NavItem};
use gds_uim::el;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    menu::ContextMenuExt,
    table::DataTable,
    tooltip::Tooltip,
};
use gpui_kit::prelude::*;

const SMALL_GAP: f32 = 4.;
const ITEM_GAP: f32 = 8.;
const CONTENT_PADDING: f32 = 24.;
pub(super) const TABLE_INSET: f32 = 18.;
const HEADER_PADDING: f32 = 16.;
pub(super) const TITLE_SIZE: f32 = 22.;
const TAB_HEIGHT: f32 = 44.;
const ITEM_RADIUS: f32 = 6.;
const CAPTION_PADDING: f32 = 6.;
const FILTER_WIDTH: f32 = 200.;
/// A collapsed tab row puts its close control in its own corner: a small X
/// must not widen the icon-only row.
const TAB_CORNER_CLOSE_SIZE: f32 = 16.;
const TAB_CORNER_CLOSE_ICON: f32 = 11.;
/// Space a tab row keeps for its close control at its trailing edge: the row's
/// inner padding, the button and the gap in front of it. A page-cycle hint ends
/// before that space, so the two keys never cover each other.
const TAB_CLOSE_RESERVE: f32 = ITEM_GAP * 2. + ui::ICON_BUTTON_SIZE;
const TERMINAL_HEIGHT: f32 = 120.;
const PANEL_PADDING: f32 = 16.;
/// Height reserved by Kit's overlay scrollbar at the table's bottom edge.
pub(super) const SCROLLBAR_LANE: f32 = DataListStyle::SCROLLBAR_LANE;
const PREVIEW_WIDTH: f32 = 280.;
const PANEL_HEADER_HEIGHT: f32 = 44.;
/// Whether the footer's left slot renders the current location scope label.
const SHOW_LOCATION_SCOPE: bool = false;

impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Take the press state from the window on every frame: a release that
        // happened while a dialog held focus never reaches this view's listener,
        // and must still hide the close controls.
        self.held_modifier = Self::current_held_modifier(window);
        self.drain_device_removals(cx);
        if self.settings_open
            || self.collection.is_some()
            || self.menu_open
            || self.keyboard_menu.is_some()
            || self.picker.is_some()
            || window.has_active_dialog(cx)
        {
            self.discard_marquee();
        }
        // The hint layer shows either the plain Ctrl keys or the keys an armed
        // prefix accepts; the mode is idempotent and only a change redraws it.
        ui::keytips::set_prefix(window, cx, self.tab_prefix.is_some());
        // Settings replaces the toolbar; every location page keeps the menu, so
        // only entering Settings drops a menu that was open over the workspace.
        if self.settings_open {
            self.menu_open = false;
        }
        let frame = Frame::new(self.width());
        let toolbar = self.render_toolbar(&frame, window, cx);
        let body = if self.settings_open {
            el! { .div { flex_1; min_h_0 } [$(self.settings_view.clone())] }.into_any_element()
        } else {
            frame
                .workspace(
                    self.render_sidebar(window, cx),
                    self.render_content(window, cx),
                    cx,
                )
                .into_any_element()
        };
        let footer = frame.footer(
            self.render_location_scope(cx),
            self.render_status(cx),
            self.render_footer_controls(window, cx),
            cx,
        );
        let panel = (!self.settings_open && self.terminal_visible)
            .then(|| self.render_terminal_placeholder(&frame, cx));
        let mut context = KeyContext::parse(if self.settings_open {
            SETTINGS_CONTEXT
        } else {
            WORKSPACE_CONTEXT
        })
        .expect("bundled key contexts are valid");
        if self.tab_prefix.is_some() {
            context.add(TAB_PREFIX_CONTEXT);
        }
        if !self.settings_open
            && (self.collection == Some(Collection::Volumes)
                || self.collection.is_none() && self.volume_pages.active(self.active_id).is_some())
        {
            context.add(VOLUME_CONTEXT);
        }
        // The picker decisions exist only on the picker page itself. Settings
        // replaces the workspace context, and a dialog owns its own context, so
        // neither can pick up `Alt+A` / `Alt+C` from here.
        if self.picker.is_some() && !self.settings_open {
            context.add(picker::PICKER_CONTEXT);
        }
        let notification_overlay = notifications::overlay(cx);
        let selection_menu = self.render_selection_menu(window);
        el! {
            .div {
                # "gds-files"; key_context: $context; track_focus: $&self.focus;
                size_full; flex; flex_col; overflow_hidden;
                text_color: $cx.theme().foreground; text_sm;
                on_action: (cx, Self::on_application_menu).Context::listener;
                on_action: (cx, Self::on_settings).Context::listener;
                on_action: (cx, Self::on_new_tab).Context::listener;
                on_action: (cx, Self::on_close_tab).Context::listener;
                on_action: (cx, Self::on_next_tab).Context::listener;
                on_action: (cx, Self::on_previous_tab).Context::listener;
                on_action: (cx, Self::on_sidebar).Context::listener;
                on_action: (cx, Self::on_terminal).Context::listener;
                on_action: (cx, Self::on_edit_location).Context::listener;
                on_action: (cx, Self::on_back).Context::listener;
                on_action: (cx, Self::on_forward).Context::listener;
                on_action: (cx, Self::on_parent).Context::listener;
                on_action: (cx, Self::on_refresh).Context::listener;
                on_action: (cx, Self::on_filter).Context::listener;
                on_action: (cx, Self::on_hidden).Context::listener;
                on_action: (cx, Self::on_open).Context::listener;
                on_action: (cx, Self::on_open_in_new_tabs).Context::listener;
                on_action: (cx, Self::on_copy_files).Context::listener;
                on_action: (cx, Self::on_cut_files).Context::listener;
                on_action: (cx, Self::on_paste_files).Context::listener;
                on_action: (cx, Self::on_copy_paths).Context::listener;
                on_action: (cx, Self::on_select_all).Context::listener;
                on_action: (cx, Self::on_deselect_all).Context::listener;
                on_action: (cx, Self::on_selection_menu).Context::listener;
                on_action: (cx, Self::on_rename).Context::listener;
                on_action: (cx, Self::on_details).Context::listener;
                on_action: (cx, Self::on_configure_columns).Context::listener;
                on_action: (cx, Self::on_favorite_current_folder).Context::listener;
                on_action: (cx, Self::on_trash).Context::listener;
                on_action: (cx, Self::on_delete_permanently).Context::listener;
                on_action: (cx, Self::on_restore_selection).Context::listener;
                on_action: (cx, Self::on_purge_selection).Context::listener;
                on_action: (cx, Self::on_empty_trash).Context::listener;
                on_action: (cx, Self::on_clear_recent_action).Context::listener;
                on_action: (cx, Self::on_mount_volume).Context::listener;
                on_action: (cx, Self::on_unmount_volume).Context::listener;
                on_action: (cx, Self::on_picker_confirm).Context::listener;
                on_action: (cx, Self::on_picker_confirm_with_control).Context::listener;
                on_action: (cx, Self::on_picker_read_only).Context::listener;
                on_action: (cx, Self::on_picker_next_filter).Context::listener;
                on_action: (cx, Self::on_picker_cancel).Context::listener;
                on_action: (cx, Self::on_tab_prefix_home).Context::listener;
                on_action: (cx, Self::on_tab_prefix_recent).Context::listener;
                on_action: (cx, Self::on_tab_prefix_favorites).Context::listener;
                on_action: (cx, Self::on_tab_prefix_network).Context::listener;
                on_action: (cx, Self::on_tab_prefix_trash).Context::listener;
                on_action: (cx, Self::on_tab_prefix_1).Context::listener;
                on_action: (cx, Self::on_tab_prefix_2).Context::listener;
                on_action: (cx, Self::on_tab_prefix_3).Context::listener;
                on_action: (cx, Self::on_tab_prefix_4).Context::listener;
                on_action: (cx, Self::on_tab_prefix_5).Context::listener;
                on_action: (cx, Self::on_tab_prefix_6).Context::listener;
                on_action: (cx, Self::on_tab_prefix_7).Context::listener;
                on_action: (cx, Self::on_tab_prefix_8).Context::listener;
                on_action: (cx, Self::on_tab_prefix_9).Context::listener;
                capture_key_down: (cx, Self::on_file_key_down).Context::listener;
                capture_key_down: (cx, Self::on_trash_key_down).Context::listener;
                capture_key_down: (cx, Self::on_place_key_down).Context::listener;
                capture_key_down: (cx, Self::on_volume_key_down).Context::listener;
                on_action: (cx, Self::on_select_up).Context::listener;
                on_action: (cx, Self::on_select_down).Context::listener;
                on_action: (cx, Self::on_select_first).Context::listener;
                on_action: (cx, Self::on_select_last).Context::listener;
                on_mouse_down: $(MouseButton::Navigate(NavigationDirection::Back)), (cx, Self::on_mouse_back).Context::listener;
                on_mouse_down: $(MouseButton::Navigate(NavigationDirection::Forward)), (cx, Self::on_mouse_forward).Context::listener;
                on_action: (cx, Self::on_cancel).Context::listener;
                capture_key_up: (cx, Self::on_tab_key_up).Context::listener;
                on_modifiers_changed: (cx, Self::on_modifiers_changed).Context::listener;
            } [toolbar, body, ..panel, footer,
                ..selection_menu,
                @if (let Some(overlay) = notification_overlay)
                    $(overlay),
                @end,
            ]
        }
    }
}

impl Browser {
    fn command_button(
        &self,
        id: &'static str,
        icon: IconName,
        command: Command,
        cx: &mut Context<Self>,
    ) -> IconButton {
        let label = i18n::text(command.label_id(), cx);
        let on_click = cx.listener(Self::click_command(command));
        el! { id.IconButton::new { icon: icon; label: label; command: command; on_click: on_click } }
    }

    fn render_toolbar(&self, frame: &Frame, window: &mut Window, cx: &mut Context<Self>) -> Div {
        // Only Settings keeps its own way back; the fixed location pages are
        // real pages with the main menu still available.
        let secondary = self.settings_open;
        let menu = if secondary {
            let on_click = cx.listener(Self::click_command(Command::Cancel));
            el! {
                "return-to-files".IconButton::new {
                    icon: IconName::ArrowLeft; label: $i18n::text("settings-return", cx);
                    command: Command::Cancel; on_click: on_click;
                }
            }
            .into_any_element()
        } else {
            menu::application_menu(self.menu_open, cx.entity().downgrade(), cx)
        };
        let sidebar = self.command_button("sidebar", IconName::PanelLeft, Command::Sidebar, cx);
        let content = if self.settings_open {
            el! { .div { flex_1; px: ITEM_GAP.px } [$i18n::text("settings", cx)] }
                .into_any_element()
        } else if let Some(collection) = self.collection {
            el! { .div { flex_1; px: ITEM_GAP.px } [$i18n::text(collection.label_id(), cx)] }
                .into_any_element()
        } else {
            self.render_navigation(window, cx)
        };
        let controls = el! {
            .div { flex; items_center; gap: SMALL_GAP.px; w_full; min_w_0 } [sidebar, content]
        };
        frame.toolbar(menu, controls.into_any_element(), cx)
    }

    fn render_navigation(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let navigation = self
            .active()
            .filter(|_| self.volume_pages.active(self.active_id).is_none())
            .and_then(|slot| slot.tab.as_ref())
            .map(|tab| &tab.navigation);
        let can_back = navigation.is_some_and(|nav| nav.can_go_back());
        let can_forward = navigation.is_some_and(|nav| nav.can_go_forward());
        let has_parent = self.current_path().parent().is_some();
        let back = self
            .command_button("back", IconName::ArrowLeft, Command::Back, cx)
            .disabled(!can_back);
        let forward = self
            .command_button("forward", IconName::ArrowRight, Command::Forward, cx)
            .disabled(!can_forward);
        let parent = self
            .command_button("parent", IconName::ArrowUp, Command::Parent, cx)
            .disabled(!has_parent);
        let path = self.render_path(window, cx);
        el! {
            .div { flex; flex_1; min_w_0; items_center; gap: SMALL_GAP.px } [back, forward, parent, path]
        }.into_any_element()
    }

    fn render_path(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.path_editing {
            return el! {
                .div { flex_1; min_w_0; px: CAPTION_PADDING.px } [
                    ($&self.path_input).Input::new { small; w_full },
                ]
            }
            .into_any_element();
        }
        let mut crumbs = Vec::new();
        for (index, segment) in breadcrumb::segments(&self.current_path())
            .into_iter()
            .enumerate()
        {
            let label: SharedString = segment.label.into();
            let on_click = cx.listener(Self::click_path(segment.path));
            crumbs.push(el! {
                ($("crumb", index)).Button::new { ghost; small; compact; label: label; on_click: on_click }
            }.into_any_element());
        }
        let edit = self.command_button("edit-path", IconName::Pencil, Command::EditLocation, cx);
        let on_edit = cx.listener(Self::on_edit_path_click);
        el! {
            .div {
                # "location-display"; cursor_text; on_click: on_edit;
                flex_1; min_w_0; flex; items_center; mx: CAPTION_PADDING.px;
                rounded: ITEM_RADIUS.px; border_1; border_color: $cx.theme().border;
            } [
                .div { # "breadcrumbs"; flex_1; min_w_0; flex; items_center; gap: SMALL_GAP.px; overflow_x_scroll } [..crumbs],
                edit,
            ]
        }.into_any_element()
    }

    pub(super) fn render_tab(
        &self,
        slot: &OpenTab,
        position: usize,
        cycle_targets: (Option<u64>, Option<u64>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = slot.id;
        let compact = self.sidebar_collapsed;
        let selected = id == self.active_id && self.collection.is_none();
        let title: SharedString = slot.title().into();
        let path: SharedString = slot.path().display().to_string().into();
        // The folder keeps its own icon; pinned rows only gain a small pin badge.
        let icon = ui::pinned_icon(IconName::Folder, slot.pinned, cx);
        let on_click = cx.listener(Self::click_tab(id));
        let on_hover = cx.listener(Self::hover_tab(id));
        let menu = Self::tab_menu(cx.entity().downgrade(), id, slot.pinned);
        // Hovering this row or holding Ctrl reveals its X; otherwise the space
        // stays reserved, so a press or release never reflows the strip.
        let show_close = self.revealing_tab_close() || self.hovered_tab == Some(id);
        let close = match (show_close, compact) {
            (true, false) => {
                let on_close = cx.listener(Self::click_close_tab(id));
                el! {
                    ($("close-tab", id)).IconButton::new {
                        icon: IconName::X; label: $i18n::text("close-this-tab", cx);
                        @if (selected) command: Command::CloseTab; @end;
                        on_click: on_close;
                    }
                }
                .into_any_element()
            }
            // A collapsed row has no free trailing space: the X sits in its own
            // top trailing corner at icon size instead of widening the row.
            (true, true) => {
                let on_close = cx.listener(Self::click_close_tab(id));
                el! {
                    .div {
                        # $("close-tab", id); absolute; top: 1.0.px; right: 1.0.px;
                    } [
                        ($("close-tab-corner", id)).Button::new {
                            ghost; small; size: TAB_CORNER_CLOSE_SIZE.px;
                            icon: IconName::X.Icon::new { size: TAB_CORNER_CLOSE_ICON.px };
                            accessibility_label: $i18n::text("close-this-tab", cx);
                            on_click: on_close;
                        },
                    ]
                }
                .into_any_element()
            }
            (false, false) => el! {
                .div {
                    # $("close-tab-slot", id);
                    w: ui::ICON_BUTTON_SIZE.px; h: ui::ICON_BUTTON_SIZE.px; flex_shrink_0;
                }
            }
            .into_any_element(),
            (false, true) => el! { .div { size_0 } }.into_any_element(),
        };
        // The cycle keys land on exactly one neighbour in each direction; those
        // two rows show them, every other numbered row shows the path to its own
        // key. While the prefix is armed only those target keys are shown.
        let adjacent = cycle_targets.0 == Some(id) || cycle_targets.1 == Some(id);
        let prefix_armed = self.tab_prefix.is_some();
        let mut hints = Vec::new();
        if !prefix_armed {
            hints.extend(self.render_tab_cycle_hints(
                id,
                cycle_targets,
                compact,
                show_close,
                window,
                cx,
            ));
        }
        if prefix_armed || !adjacent {
            hints.extend(
                self.render_tab_ordinal_hint(id, position, compact, show_close, window, cx),
            );
        }
        let muted = cx.theme().muted_foreground;
        let tooltip = Self::tab_tooltip(title.clone(), path.clone(), muted);
        let tab = el! {
            .div {
                # $("tab", id); relative; flex; items_center; gap: ITEM_GAP.px; w_full; flex_shrink_0;
                @if (compact) h: ui::SIDEBAR_ITEM_HEIGHT.px; min_h: ui::SIDEBAR_ITEM_HEIGHT.px; justify_center;
                @else min_h: TAB_HEIGHT.px; @end;
                px: ITEM_GAP.px; rounded: ITEM_RADIUS.px; cursor_pointer;
                @if (selected) bg: $cx.theme().selection;
                @else hover: { bg: $cx.theme().secondary }; @end;
                on_click: on_click;
                on_hover: on_hover;
                @if (!self.revealing_tab_close()) tooltip_show_delay: ui::TOOLTIP_DELAY; tooltip: tooltip; @end;
            } [
                icon,
                @if (!compact)
                    .div { flex_1; min_w_0; flex; flex_col } [
                        .div { text_ellipsis } [title],
                        .div { text_xs; text_color: $cx.theme().muted_foreground; text_ellipsis } [path],
                    ],
                    $(close),
                @else
                    $(close),
                @end,
                ..hints,
            ]
        };
        let tab = drag_drop::tab_drop_target(
            tab,
            slot.path().to_path_buf(),
            id,
            cx.entity().downgrade(),
            window,
            cx,
        );
        el! { tab { context_menu: menu; into_any_element } }
    }

    /// A row's folder name and its whole path.
    fn tab_tooltip(
        title: SharedString,
        path: SharedString,
        muted: Hsla,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView {
        move |window, cx| {
            let title = title.clone();
            let path = path.clone();
            Tooltip::element(move |_, _| {
                el! {
                    .div { flex; flex_col; gap: 2.0.px; max_w: 420.0.px } [
                        .div { font_weight: FontWeight::SEMIBOLD; whitespace_normal } [$title.clone()],
                        .div { text_xs; text_color: $(muted); whitespace_normal } [$path.clone()],
                    ]
                }
            })
            .build(window, cx)
        }
    }

    /// The page-cycle keys staged on this tab row.
    ///
    /// A row carries a label only when a cycle key really lands on it: the same
    /// target [`Browser::cycle_tab`] uses, so the two cannot disagree.
    fn render_tab_cycle_hints(
        &self,
        id: u64,
        cycle_targets: (Option<u64>, Option<u64>),
        compact: bool,
        show_close: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        let (previous, next) = cycle_targets;
        let mut hints = Vec::new();
        for (backwards, target) in [(true, previous), (false, next)] {
            if target != Some(id) {
                continue;
            }
            let (key, command) = if backwards {
                ("tab-previous-hint", Command::PreviousTab)
            } else {
                ("tab-next-hint", Command::NextTab)
            };
            let key: ElementId = (key, id).into();
            hints.push(Self::stage_tab_hint(
                key,
                ui::Shortcut::from(command),
                compact,
                show_close,
                window,
                cx,
            ));
        }
        hints
    }

    /// The ordinal key staged on this tab row.
    ///
    /// The ordinal is the row's position in the strip, pinned tabs included;
    /// rows past the ninth have no key, so they stage nothing.
    fn render_tab_ordinal_hint(
        &self,
        id: u64,
        position: usize,
        compact: bool,
        show_close: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let shortcut = tabs::ordinal_shortcut(position)?;
        Some(Self::stage_tab_hint(
            ("tab-ordinal-hint", id).into(),
            shortcut,
            compact,
            show_close,
            window,
            cx,
        ))
    }

    /// Stage a tab row's label where the row has room for it.
    ///
    /// A collapsed row has no trailing space and keeps its close control in its
    /// top trailing corner, so its label takes the free bottom leading corner
    /// rather than being dropped with the space it would have used.
    fn stage_tab_hint(
        key: ElementId,
        shortcut: ui::Shortcut,
        compact: bool,
        show_close: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        match (compact, show_close) {
            (true, true) => ui::keytips::bottom_corner_anchor(window, cx, key, shortcut),
            (true, false) => ui::keytips::anchor(window, cx, key, shortcut),
            (false, _) => ui::keytips::trailing_before_anchor(
                window,
                cx,
                key,
                shortcut,
                px(TAB_CLOSE_RESERVE),
            ),
        }
    }

    pub(super) fn render_location(
        &self,
        target: LocationTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let on_click = cx.listener(Self::click_location(target));
        let id = target.label_id();
        let icon = target.icon();
        let label = i18n::text(id, cx);
        let nav = el! {
            (id, icon, label).NavItem::new {
                compact: $self.sidebar_collapsed;
                selected: $self.location_selected(target);
                on_click: on_click;
            }
        };
        // The page key is staged in both layers: the plain one shows the path to it
        // (`Tab+H`), the armed prefix the target alone (`H`).
        let hint = ui::keytips::row_anchor(
            window,
            cx,
            format!("location-prefix-hint-{id}").into(),
            tabs::location_shortcut(target),
        );
        el! { .div { relative; w_full } [nav, hint] }.into_any_element()
    }

    /// The footer's far-left slot reports the current location scope.
    ///
    /// It is hidden for now. The slot and the footer geometry stay in place so
    /// no other control moves; only this label is suppressed. Bringing it back
    /// is a source change, not a user setting.
    fn render_location_scope(&self, cx: &App) -> AnyElement {
        if !SHOW_LOCATION_SCOPE {
            return el! { .div { # "location-scope"; size_full } }.into_any_element();
        }
        el! {
            .div {
                # "location-scope"; size_full; flex; items_center;
                text_xs; text_color: $cx.theme().muted_foreground;
                @if (self.sidebar_collapsed) justify_center; @else px: CONTENT_PADDING.px; @end;
            } [
                @if (!self.settings_open) $i18n::text("local", cx), @end,
            ]
        }
        .into_any_element()
    }

    fn render_content_header(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let title: SharedString = if self.home_active() {
            // Home is a fixed page, so its heading names the page rather than
            // repeating the folder name the breadcrumbs already show.
            i18n::text("home", cx)
        } else {
            self.active().map(OpenTab::title).unwrap_or_default().into()
        };
        let favorite = self.places.is_favorite(&self.current_path());
        let on_favorite = cx.listener(Self::on_favorite);
        let hidden = self
            .command_button("hidden", IconName::Eye, Command::Hidden, cx)
            .active(self.preferences.files.show_hidden);
        let refresh = self.command_button("refresh", IconName::RefreshCw, Command::Refresh, cx);
        let details = self
            .command_button("details", IconName::Info, Command::Details, cx)
            .active(self.preview_visible);
        let on_columns = cx.listener(Self::on_columns);
        // The filter box itself is the real target of its binding.
        let filter_hint = ui::keytips::row_anchor(
            window,
            cx,
            "filter-hint".into(),
            ui::Shortcut::from(Command::Filter),
        );
        el! {
            .div { flex; items_center; gap: ITEM_GAP.px; px: CONTENT_PADDING.px; py: HEADER_PADDING.px } [
                .div { flex_1; min_w_0; text_size: $(gds::typography::scaled(TITLE_SIZE, cx)); font_weight: FontWeight::SEMIBOLD; truncate } [title],
                .div { relative; w: FILTER_WIDTH.px; flex_shrink_0 } [
                    ($&self.filter_input).Input::new { small; cleanable: true; prefix: IconName::Search.Icon::new },
                    filter_hint,
                ],
                "favorite".IconButton::new {
                    icon: IconName::Star;
                    label: $i18n::text(if favorite { "favorite-remove" } else { "favorite-add" }, cx);
                    active: favorite;
                    command: Command::Favorite;
                    on_click: on_favorite;
                },
                "columns".IconButton::new {
                    icon: IconName::Columns3Cog; label: $i18n::text("columns-configure", cx);
                    command: Command::Columns;
                    disabled: $(!self.columns_available()); on_click: on_columns;
                },
                hidden, details, refresh,
            ]
        }
    }

    fn render_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if let Some(collection) = self.collection {
            return self.render_collection(collection, window, cx);
        }
        if self.active().is_some_and(|slot| !slot.files_available()) {
            return self.render_device_unavailable(cx);
        }
        self.sync_preview(cx);
        let header = self.render_content_header(window, cx);
        let list = self.render_file_table(window, cx);
        let menu = Self::directory_menu_builder(cx.entity().downgrade(), self.current_path());
        let list_area = el! {
            .div { # "file-list-area"; flex_1; min_h_0; overflow_hidden; px: TABLE_INSET.px } [list]
            { context_menu: menu; into_any_element }
        };
        let rows = self.active().map_or(0, |slot| slot.table_rows.len());
        let scroll = self
            .active()
            .map(|slot| slot.scroll.0.borrow().base_handle.clone())
            .unwrap_or_default();
        let list_area =
            ui::ListSurface::new("directory-selection-surface", list_area, rows, scroll)
                .inset_x(px(TABLE_INSET))
                .header_height(px(if rows == 0 {
                    0.
                } else {
                    DataListStyle::HEADER_HEIGHT
                }))
                .on_blank_press(cx.listener(Self::on_list_background_press))
                .into_any_element();
        // A picker holds one path at a time, so its list has no rubber-band
        // surface: a batch selection could neither be reported nor kept.
        let list_area = if self.picker.is_some() {
            el! {
                .div { # "file-list-plain"; flex; flex_col; flex_1; min_h_0; min_w_0; overflow_hidden } [list_area]
            }
            .into_any_element()
        } else {
            self.marquee_surface(list_area.into_any_element(), window, cx)
        };
        let content = el! {
            .div { flex_1; min_w_0; flex; flex_col; min_h_0 } [
                header,
                list_area,
            ]
        };
        let panel = self.preview_visible.then(|| {
            el! {
                .div {
                    # "preview-panel"; w: PREVIEW_WIDTH.px; flex_shrink_0; h_full; min_h_0;
                    flex; flex_col; overflow_hidden; border_l_1; border_color: $cx.theme().border;
                } [
                    .div {
                        flex; items_center; flex_shrink_0; h: PANEL_HEADER_HEIGHT.px;
                        px: PANEL_PADDING.px; gap: ITEM_GAP.px;
                    } [
                        IconName::Info.Icon::new { size: ui::SIDEBAR_ICON_SIZE.px;
                            text_color: $(cx.theme().muted_foreground) },
                        .div { flex_1; min_w_0; text_xs; font_weight: FontWeight::SEMIBOLD;
                            text_color: $(cx.theme().muted_foreground); } [$i18n::text("details-title", cx)],
                    ],
                    .div { flex_1; min_h_0; min_w_0; flex; flex_col; overflow_hidden } [$(self.preview.clone())],
                ]
            }
        });
        // The workspace content column is not a flex parent, so the row has to fill
        // the available height itself for the list area to receive any space.
        el! {
            .div { flex; size_full; min_h_0; min_w_0 } [content, ..panel]
        }
        .into_any_element()
    }

    fn render_file_table(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(slot) = self.active().filter(|slot| slot.tab.is_some()) else {
            let loading = self.active().is_some_and(OpenTab::is_loading);
            return self
                .empty_file_list(
                    i18n::text(if loading { "loading" } else { "load-failed" }, cx),
                    cx,
                )
                .into_any_element();
        };
        let navigation = &slot.tab.as_ref().expect("loaded tab").navigation;
        if slot.visible.is_empty() {
            let message = if !slot.filter.is_empty() {
                "empty-filter"
            } else if slot
                .picker
                .as_ref()
                .is_some_and(picker::Projection::narrows)
            {
                "picker-empty-files"
            } else {
                "empty-directory"
            };
            return self
                .empty_file_list(i18n::text(message, cx), cx)
                .into_any_element();
        }
        let data = table::Data {
            rows: slot.table_rows.clone(),
            selected: navigation.view().selected.clone(),
            sort: slot.sort,
            scroll: slot.scroll.clone(),
            content_width: f32::from(window.bounds().size.width)
                - self.width()
                - TABLE_INSET * 2.
                - if self.preview_visible {
                    PREVIEW_WIDTH
                } else {
                    0.
                },
            layout: self.columns_layout(&self.current_path()).clone(),
        };
        table::FileTable::sync_state(&self.file_table, data, cx);
        let height =
            DataListStyle::HEADER_HEIGHT + DataListStyle::ROW_HEIGHT * slot.visible.len() as f32;
        let table = el! {
            ($&self.file_table).DataTable::new {
                bordered: false;
                with_size: $(gpui_kit::component::Size::Size(px(DataListStyle::ROW_HEIGHT)));
                header_row_height: DataListStyle::HEADER_HEIGHT.px;
                into_any_element;
            }
        };
        let empty_space = el! {
            .div {
                # "directory-empty-space"; absolute; left_0; right_0;
                top: height.px; bottom: SCROLLBAR_LANE.px;
            }
        };
        let empty_menu = Self::directory_menu_builder(cx.entity().downgrade(), self.current_path());
        let empty_space =
            drag_drop::drop_target(empty_space, self.current_path(), cx.entity().downgrade())
                .context_menu(empty_menu);
        // The table owns the full content viewport so its horizontal scrollbar
        // stays at the bottom instead of riding under the last short row. The
        // blank area below the rows remains a separate target, inset by the
        // scrollbar lane so the scrollbar itself stays draggable.
        el! {
            .div { # "file-table-area"; relative; flex; flex_col; size_full; min_h_0 } [
                .div { flex_1; min_h_0; min_w_0 } [table],
                empty_space,
            ]
        }
        .into_any_element()
    }

    fn empty_file_list(&self, message: SharedString, cx: &mut Context<Self>) -> Stateful<Div> {
        let empty = el! {
            .div { # "empty-file-list"; size_full; flex; items_center; justify_center;
            } [message]
        };
        drag_drop::drop_target(empty, self.current_path(), cx.entity().downgrade())
    }

    fn empty_message(&self, message: SharedString) -> Div {
        el! { .div { size_full; flex; items_center; justify_center } [message] }
    }

    fn render_collection(
        &self,
        collection: Collection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Trash and Network are virtual locations; they never borrow a
        // filesystem path from the active tab.
        match collection {
            Collection::Volumes => return self.render_devices_page(window, cx),
            Collection::Trash => return self.render_trash(window, cx),
            Collection::Network => return self.render_network(cx),
            Collection::Recent | Collection::Favorites => {}
        }
        let Some(cursor) = self.place_cursor(collection) else {
            return self
                .empty_message(i18n::text("empty-collection", cx))
                .into_any_element();
        };
        let paths = collection.place_paths(&self.places).unwrap_or(&[]);
        let selected = cursor
            .path
            .as_deref()
            .and_then(|path| paths.iter().position(|entry| Path::new(entry) == path));
        let icon = collection.icon();
        let rows: Vec<_> = paths
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let path = PathBuf::from(entry);
                let label = path.display().to_string();
                let menu = Self::place_menu(cx.entity().downgrade(), collection, path.clone());
                let on_click = cx.listener(Self::click_place(path));
                let row = el! {
                    ($("place", index), icon, label).NavItem::new {
                        selected: $(selected == Some(index));
                        on_click: on_click;
                    }
                };
                el! { .div { # $("place-context", index); w_full; flex_shrink_0 } [row] }
                    .context_menu(menu)
                    .into_any_element()
            })
            .collect();
        let title: SharedString = i18n::text(collection.label_id(), cx);
        let mut actions = Vec::new();
        if collection == Collection::Recent {
            // Only a history can be cleared; the control stays disabled rather
            // than pretending an empty list has something to drop.
            let on_clear = cx.listener(Self::on_clear_recent);
            actions.push(
                el! {
                    "clear-recent".IconButton::new {
                        icon: IconName::Broom; label: $i18n::text("clear-recent", cx);
                        command: Command::ClearRecent; disabled: $(paths.is_empty());
                        on_click: on_clear;
                    }
                }
                .into_any_element(),
            );
        }
        let list: AnyElement = if paths.is_empty() {
            self.empty_message(i18n::text("empty-collection", cx))
                .into_any_element()
        } else {
            el! {
                .div {
                    # "places-list"; flex_1; min_h_0; overflow_y_scroll;
                    track_scroll: $(&cursor.scroll);
                } [..rows]
            }
            .into_any_element()
        };
        let body = el! {
            .div {
                # "places-body"; flex_1; min_h_0; flex; flex_col;
                px: CONTENT_PADDING.px; pb: CONTENT_PADDING.px;
            } [list]
        }
        .into_any_element();
        let body = ui::ListSurface::new(
            "places-selection-surface",
            body,
            paths.len(),
            cursor.scroll.clone(),
        )
        .row_height(px(ui::SIDEBAR_ITEM_HEIGHT))
        .header_height(px(0.))
        .inset_x(px(CONTENT_PADDING))
        .bottom_inset(px(0.))
        .on_blank_press(cx.listener(Self::on_list_background_press));
        el! {
            .div { # "places-content"; flex; flex_col; size_full; min_h_0 } [
                .div {
                    # "places-header"; flex; flex_shrink_0; items_center; gap: ITEM_GAP.px;
                    px: CONTENT_PADDING.px; py: HEADER_PADDING.px;
                } [
                    .div {
                        flex_1; min_w_0; truncate; font_weight: FontWeight::SEMIBOLD;
                        text_size: $(gds::typography::scaled(TITLE_SIZE, cx));
                    } [title],
                    ..actions,
                ],
                body,
            ]
        }
        .into_any_element()
    }

    /// The Network page is honest about not browsing yet; it is still a fixed,
    /// selectable page with the same menu and its own prefix key.
    fn render_network(&self, cx: &App) -> AnyElement {
        el! {
            .div { size_full; flex; flex_col; p: CONTENT_PADDING.px; gap: TABLE_INSET.px } [
                .div { text_lg; font_weight: FontWeight::SEMIBOLD } [$i18n::text("network", cx)],
                $self.empty_message(i18n::text("network-not-implemented", cx)),
            ]
        }
        .into_any_element()
    }

    fn render_terminal_placeholder(&self, frame: &Frame, cx: &mut Context<Self>) -> AnyElement {
        let compact = self.sidebar_collapsed;
        let on_open = cx.listener(Self::click_external_terminal(self.current_path()));
        let sidebar = el! {
            .div { p: ui::SIDEBAR_PADDING.px } [
                .div { flex; items_center; h: ui::SIDEBAR_ITEM_HEIGHT.px; gap: ITEM_GAP.px; px: ITEM_GAP.px;
                    @if (compact) justify_center; @end;
                } [
                    IconName::Terminal.Icon::new { size: ui::SIDEBAR_ICON_SIZE.px },
                    @if (!compact) $i18n::text("settings-terminal", cx), @end,
                ],
            ]
        };
        let content = el! {
            .div { flex; size_full; min_w_0 } [
                .div { flex_1; min_w_0; p: PANEL_PADDING.px; flex; flex_col; gap: ITEM_GAP.px } [
                    .div { text_color: $cx.theme().muted_foreground } [$i18n::text("terminal-not-ready", cx)],
                    "external-terminal".Button::new { small; label: $i18n::text("terminal-open-external", cx); icon: IconName::Terminal.Icon::new; on_click: on_open },
                ],
            ]
        };
        let panel = frame.workspace(sidebar.into_any_element(), content.into_any_element(), cx);
        el! { panel { flex_none; h: TERMINAL_HEIGHT.px; border_t_1; border_color: $cx.theme().border; into_any_element } }
    }

    /// Window-level toggles live in the footer, never duplicated in the toolbar.
    ///
    /// A picker replaces the terminal toggle with its own decisions, so its
    /// filter group, read-only mark, cancel and confirm sit where the terminal
    /// control sits in a browser window; leaving picker mode restores it.
    fn render_footer_controls(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.settings_open {
            return el! { .div }.into_any_element();
        }
        if self.picker.is_some() {
            return self.render_picker_controls(window, cx);
        }
        let terminal = self
            .command_button("terminal", IconName::Terminal, Command::Terminal, cx)
            .active(self.terminal_visible);
        el! { .div { flex; items_center; flex_shrink_0 } [terminal] }.into_any_element()
    }

    fn render_status(&self, cx: &App) -> AnyElement {
        let mut labels = Vec::new();
        if self.settings_open {
            labels.push(i18n::text("settings-auto-apply", cx));
        } else if self.collection == Some(Collection::Volumes) {
            let count = self
                .storage
                .read(cx)
                .snapshot
                .volumes
                .iter()
                .filter(|volume| volume.visible)
                .count();
            labels.push(i18n::count("volume-count", count, cx));
        } else if self.collection == Some(Collection::Trash) {
            if self.trash.is_loading() {
                labels.push(i18n::text("recycle-loading", cx));
            }
            labels.push(i18n::count("item-count", self.trash.rows().len(), cx));
            let selected = self.trash.selected_count();
            if selected > 0 {
                labels.push(i18n::count("selection-count", selected, cx));
            }
        } else if let Some(slot) = self.active().filter(|slot| slot.files_available())
            && let Some(tab) = &slot.tab
        {
            let directory = tab.navigation.directory();
            if slot.is_loading() {
                labels.push(i18n::text("loading", cx));
            }
            if slot.filter.is_empty() {
                if slot
                    .picker
                    .as_ref()
                    .is_some_and(picker::Projection::narrows)
                {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("visible", slot.visible.len() as i64);
                    args.set("total", directory.entries.len() as i64);
                    labels.push(i18n::format("filter-count", Some(&args), cx));
                } else {
                    labels.push(i18n::count("item-count", directory.entries.len(), cx));
                }
            } else {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("visible", slot.visible.len() as i64);
                args.set("total", directory.entries.len() as i64);
                labels.push(i18n::format("filter-count", Some(&args), cx));
            }
            let selected = tab.navigation.view().selected.len();
            if selected > 0 {
                labels.push(i18n::count("selection-count", selected, cx));
            }
        }
        el! {
            .div { flex; items_center; gap: TABLE_INSET.px; min_w_0; text_xs; text_color: $cx.theme().muted_foreground } [
                ..labels,
            ]
        }.into_any_element()
    }
}

impl LocationTarget {
    pub(super) const ALL: [Self; 5] = [
        Self::Home,
        Self::Recent,
        Self::Favorites,
        Self::Network,
        Self::Trash,
    ];
    fn label_id(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Recent => "recent",
            Self::Favorites => "favorites",
            Self::Network => "network",
            Self::Trash => "trash",
        }
    }
    fn icon(self) -> IconName {
        match self {
            Self::Home => IconName::House,
            Self::Recent => IconName::Clock,
            Self::Favorites => IconName::Star,
            Self::Network => IconName::Network,
            Self::Trash => IconName::Trash,
        }
    }
}

impl Collection {
    fn label_id(self) -> &'static str {
        match self {
            Self::Volumes => "volumes",
            Self::Recent => "recent",
            Self::Favorites => "favorites",
            Self::Network => "network",
            Self::Trash => "trash",
        }
    }
    fn icon(self) -> IconName {
        match self {
            Self::Volumes => IconName::HardDrive,
            Self::Recent => IconName::Clock,
            Self::Favorites => IconName::Star,
            Self::Network => IconName::Network,
            Self::Trash => IconName::Trash,
        }
    }
    /// Places listed directly by a collection; `None` for a virtual location
    /// whose rows come from a backend instead of the saved places.
    pub(super) fn place_paths(self, places: &Places) -> Option<&[std::ffi::OsString]> {
        match self {
            Self::Recent => Some(&places.recent),
            Self::Favorites => Some(&places.favorites),
            Self::Network | Self::Trash | Self::Volumes => None,
        }
    }
}
