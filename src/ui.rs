//! Shared window geometry, controls and shortcut presentation.

use std::{rc::Rc, time::Duration};

use gds_uim::el;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable};
use gpui_kit::{prelude::*, *};

use crate::{actions::Command, theme, typography};

pub mod button;
pub mod capacity;
pub mod dialog;
pub mod keytips;
pub mod list;
pub mod page;
pub mod region;
pub mod settings;
mod shortcut;
pub mod split;
pub mod tooltip;
pub mod window;
pub use dialog::DialogActions;
pub use list::ListSurface;
pub use page::ContentPage;
use shortcut::Hint;
pub use shortcut::{
    HeldModifier, PrefixTarget, Shortcut, ShortcutBadge, command_tooltip, shortcut_tooltip,
};

pub use crate::design::{
    COLLAPSED_SIDEBAR_WIDTH, FOOTER_HEIGHT, ICON_BUTTON_SIZE, SIDEBAR_ICON_SIZE,
    SIDEBAR_ITEM_HEIGHT, SIDEBAR_PADDING, SIDEBAR_WIDTH, TOOLBAR_HEIGHT,
};
pub const SMALL: Rems = Rems(crate::design::UI_TEXT_SCALE);
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(400);
const ITEM_GAP: f32 = 8.0;
const ITEM_RADIUS: f32 = 6.0;
const KEY_HEIGHT: f32 = 22.0;
const KEY_GAP: f32 = 4.0;
const KEY_PADDING: f32 = 6.0;
/// Matches the previous `text_xs` (0.75rem at the default 16px base).
const KEY_TEXT_SIZE: f32 = 12.0;
const HINT_GAP: f32 = 16.0;
const MENU_PADDING: f32 = 12.0;
const STATUS_PADDING: f32 = 18.0;
const DIVIDER_WIDTH: f32 = 1.0;
/// Smallest toolbar stretch still worth offering as a window move handle. A
/// collapsed sidebar leaves only a sliver beside the menu button, and pressing
/// that would be an accident rather than a drag.
const MIN_DRAG_WIDTH: f32 = 8.0;
/// The pin badge over an icon's corner; small enough to leave the icon legible.
const PIN_BADGE_SIZE: f32 = 10.0;

/// Shared density and hierarchy for content lists, separate from navigation rows.
pub struct DataListStyle;

impl DataListStyle {
    pub const ROW_HEIGHT: f32 = 40.0;
    pub const HEADER_HEIGHT: f32 = 30.0;
    pub const ROW_RADIUS: f32 = 6.0;
    pub const SELECTION_INSET: f32 = 2.0;
    pub const NAME_SIZE: f32 = 13.0;
    pub const META_SIZE: f32 = 12.0;
    pub const HEADER_SIZE: f32 = 11.0;
    pub const CELL_PADDING_X: f32 = 12.0;
    pub const CELL_PADDING_Y: f32 = 4.0;
    pub const SCROLLBAR_LANE: f32 = 16.0;
}

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    button: Button,
    label: SharedString,
    command: Option<Shortcut>,
    /// A binding outside the workspace command set; it wins when both are set.
    shortcut: Option<Shortcut>,
    disabled: bool,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        let button = el! { (~id).Button::new { ghost; small; size: ICON_BUTTON_SIZE.px } };
        Self {
            id,
            button,
            label: "".into(),
            command: None,
            shortcut: None,
            disabled: false,
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.button = self
            .button
            .icon(Icon::new(icon).size(px(crate::design::ICON_SIZE)));
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self.button = self.button.accessibility_label(self.label.clone());
        self
    }

    pub fn command(mut self, command: impl Into<Shortcut>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// The binding this control runs outside the workspace command set.
    ///
    /// Its hint and tooltip then read that binding exactly like a command
    /// control, so a key is never advertised from the wrong scope.
    pub fn shortcut(mut self, shortcut: Shortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.button = self.button.disabled(disabled);
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        if active {
            self.button = self.button.primary();
        }
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.button = self.button.on_click(handler);
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            button,
            label,
            command,
            shortcut,
            disabled,
        } = self;
        let shortcut = shortcut.or(command);
        // A disabled control cannot run its action, so it never advertises a key.
        let hint = match &shortcut {
            Some(shortcut) if !disabled => {
                Some(keytips::anchor(window, cx, id.clone(), shortcut.clone()))
            }
            _ => None,
        };
        let tooltip = match shortcut {
            Some(shortcut) => Hint::shortcut(label, shortcut),
            None => Hint::command(label, None),
        }
        .builder();
        // The hint's containing block is the button, not a stretched parent row.
        el! {
            .div { # id; relative; size: ICON_BUTTON_SIZE.px; flex_shrink_0; tooltip_show_delay: TOOLTIP_DELAY; tooltip: tooltip } [
                button,
                @if (let Some(hint) = hint) $(hint), @end,
            ]
        }
    }
}

/// Navigation rows supply their own full-width content to Kit's Button.
/// This avoids its independently centered label container.
#[derive(IntoElement)]
pub struct NavItem {
    id: ElementId,
    button: Button,
    icon: IconName,
    label: SharedString,
    compact: bool,
    command: Option<Shortcut>,
    shortcut: Option<Shortcut>,
    disabled: bool,
    tooltip_text: Option<SharedString>,
    tooltip_enabled: bool,
    details: Option<AnyElement>,
    trailing: Vec<AnyElement>,
}

impl NavItem {
    pub fn new(id: impl Into<ElementId>, icon: IconName, label: impl Into<SharedString>) -> Self {
        let id = id.into();
        let label = label.into();
        let button =
            el! { (~id).Button::new { ghost; text_size: SMALL; accessibility_label: ~label } };
        Self {
            id,
            button,
            icon,
            label,
            compact: false,
            command: None,
            shortcut: None,
            disabled: false,
            tooltip_text: None,
            tooltip_enabled: true,
            details: None,
            trailing: Vec::new(),
        }
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
    /// Keep the row compact while describing state and properties on hover.
    pub fn tooltip_text(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip_text = Some(text.into());
        self
    }
    pub fn tooltip_enabled(mut self, enabled: bool) -> Self {
        self.tooltip_enabled = enabled;
        self
    }
    /// Secondary content shares the row's hover, selection and click target.
    pub fn details(mut self, details: impl IntoElement) -> Self {
        self.details = Some(details.into_any_element());
        self
    }
    /// Standard icon-sized controls stay independent of the navigation button.
    pub fn trailing(mut self, controls: Vec<AnyElement>) -> Self {
        self.trailing = controls;
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.button = self.button.selected(selected);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.button = self.button.disabled(disabled);
        self
    }
    pub fn command(mut self, command: impl Into<Shortcut>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// The binding this row performs outside the file workspace, where the
    /// workspace command list does not apply.
    pub fn shortcut(mut self, shortcut: Shortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.button = self.button.on_click(handler);
        self
    }
}

impl RenderOnce for NavItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            button,
            icon,
            label,
            compact,
            command,
            shortcut,
            disabled,
            tooltip_text,
            tooltip_enabled,
            details,
            trailing,
        } = self;
        let trailing_width = trailing.len() as f32 * ICON_BUTTON_SIZE;
        let header_right_padding = px(ITEM_GAP + trailing_width);
        let trailing_top = px((SIDEBAR_ITEM_HEIGHT - ICON_BUTTON_SIZE) / 2.);
        let trailing: Vec<_> = trailing.into_iter().map(|control| {
            el! { .div { size: ICON_BUTTON_SIZE.px; flex; items_center; justify_center } [control] }
        }).collect();
        let has_details = details.is_some();
        let content = el! {
            .div {
                flex; flex_col; w_full; min_w_0;
            } [
                .div { flex; w_full; min_w_0; flex_shrink_0; items_center; gap: ITEM_GAP.px;
                    h: SIDEBAR_ITEM_HEIGHT.px; px: ITEM_GAP.px;
                    @if (compact) justify_center; @else pr: header_right_padding; @end;
                } [
                    icon.Icon::new { size: SIDEBAR_ICON_SIZE.px; flex_shrink_0 },
                    @if (!compact) .div { min_w_0; text_ellipsis } [~label], @end,
                ],
                ..details,
            ]
        };
        let button = el! {
            button { w_full; p_0; rounded: ITEM_RADIUS.px;
                @if (has_details) h_auto; @else h: SIDEBAR_ITEM_HEIGHT.px; @end;
            } [content]
        };
        let shortcut = shortcut.or(command);
        // A full-width row shows its key in its own trailing space, where it
        // covers no other row; a collapsed row has no such space, so its label
        // becomes a corner badge on the row itself.
        let hint = match &shortcut {
            Some(shortcut) if !disabled => Some(if compact {
                keytips::anchor(window, cx, id.clone(), shortcut.clone())
            } else {
                keytips::row_anchor(window, cx, id.clone(), shortcut.clone())
            }),
            _ => None,
        };
        let tooltip_text = tooltip_text.unwrap_or_else(|| label.clone());
        let tooltip = match shortcut {
            Some(shortcut) => Hint::shortcut(tooltip_text, shortcut),
            None => Hint::command(tooltip_text, None),
        }
        .builder();
        el! {
            .div {
                # id; relative; w_full; flex_shrink_0;
                @if (!has_details) h: SIDEBAR_ITEM_HEIGHT.px; @end;
                @if (tooltip_enabled) tooltip_show_delay: TOOLTIP_DELAY; tooltip: tooltip; @end;
            } [
                button,
                @if (!trailing.is_empty())
                    .div { absolute; right_0; top: trailing_top; flex } [
                        ..trailing,
                    ],
                @end,
                @if (let Some(hint) = hint) $(hint), @end,
            ]
        }
    }
}

/// A fixed trailing slot that replaces a passive status with an action on row
/// hover. Neither presentation changes the space available to the row's title.
pub fn hover_action(
    group: SharedString,
    status: AnyElement,
    action: Option<AnyElement>,
) -> AnyElement {
    let has_action = action.is_some();
    el! {
        .div { relative; flex_shrink_0; size: ICON_BUTTON_SIZE.px } [
            .div {
                size_full; flex; items_center; justify_center;
                when: has_action, {
                    group_hover: $(group.clone()), $(gds_uim::m! { invisible; });
                };
            } [status],
            @if (let Some(action) = action)
                .div {
                    absolute; inset_0; size_full; invisible;
                    group_hover: group, $(gds_uim::m! { visible; });
                } [action],
            @end,
        ]
    }
    .into_any_element()
}

/// A row's own icon, marked with a corner badge when the row is pinned.
///
/// The icon keeps its identity and its shared size whether or not the row is
/// pinned: replacing it with a pin would drop what the row points at, and
/// resizing it would shift the row's label between the two states.
pub fn pinned_icon(icon: IconName, pinned: bool, cx: &App) -> AnyElement {
    with_pin(Icon::new(icon).size(px(SIDEBAR_ICON_SIZE)), pinned, cx)
}

/// Apply the same corner marker to an application-provided navigation icon.
pub fn with_pin(icon: impl IntoElement, pinned: bool, cx: &App) -> AnyElement {
    el! {
        .div { relative; flex_shrink_0; size: SIDEBAR_ICON_SIZE.px } [
            $(icon),
            @if (pinned)
                .div {
                    absolute; right_0; bottom_0;
                    flex; items_center; justify_center;
                    size: PIN_BADGE_SIZE.px;
                } [
                    IconName::Pin.Icon::new {
                        size: PIN_BADGE_SIZE.px; text_color: $(cx.theme().muted_foreground),
                    },
                ],
            @end,
        ]
    }
    .into_any_element()
}

/// The empty stretch beside the toolbar's leading control, which moves the
/// window.
///
/// The window has no native title bar, so this filler carries the interactive
/// move through the platform's own [`Window::start_window_move`]. It is a
/// sibling of the menu button and of the toolbar controls, never a wrapper, so
/// pressing those still only clicks them.
#[derive(IntoElement)]
pub struct WindowDragArea;

impl RenderOnce for WindowDragArea {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        el! {
            .div {
                # "toolbar-drag-area"; flex_1; h_full;
                on_mouse_down: MouseButton::Left, $|_, window, _| window.start_window_move();
            }
        }
    }
}

/// One shared column width governs toolbar, workspace and footer geometry.
pub struct Frame {
    sidebar_width: Option<f32>,
}

impl Frame {
    pub fn new(sidebar_width: f32) -> Self {
        Self {
            sidebar_width: Some(sidebar_width),
        }
    }

    /// A full-width page whose toolbar has no sidebar column or divider.
    pub fn without_sidebar() -> Self {
        Self {
            sidebar_width: None,
        }
    }

    fn vertical_divider(&self, cx: &App) -> Option<Div> {
        self.sidebar_width.map(|boundary| el! {
            .div { absolute; left: boundary.px; top_0; bottom_0; w: DIVIDER_WIDTH.px; bg: $(cx.theme().border) }
        })
    }

    /// The toolbar's empty stretch after its leading control, when the column
    /// is wide enough to leave one.
    ///
    /// The leading control is an [`ICON_BUTTON_SIZE`] control in both of the
    /// toolbar's forms (the menu and the secondary page's way back), so the
    /// space beside it is the column width minus the control and its padding.
    fn window_drag_area(&self) -> Option<WindowDragArea> {
        let free = self.sidebar_width? - 2. * MENU_PADDING - ICON_BUTTON_SIZE;
        (free >= MIN_DRAG_WIDTH).then_some(WindowDragArea)
    }

    /// The window's top row: the menu column, the page controls, and the
    /// column divider the workspace and footer draw at the same coordinate.
    ///
    /// The empty stretch beside the menu moves the window; the menu and the
    /// controls stay ordinary click targets.
    pub fn toolbar(&self, menu: AnyElement, controls: AnyElement, cx: &App) -> Div {
        self.toolbar_with_window_controls(menu, controls, window::WindowControls::new(), cx)
    }

    /// Applications supply lifecycle callbacks; Frame owns the placement so a
    /// full-width title or navigation row cannot displace the window controls.
    pub fn toolbar_with_window_controls(
        &self,
        menu: AnyElement,
        controls: AnyElement,
        window_controls: window::WindowControls,
        cx: &App,
    ) -> Div {
        let width = self
            .sidebar_width
            .unwrap_or(2. * MENU_PADDING + ICON_BUTTON_SIZE);
        let leading_background = if self.sidebar_width.is_some() {
            theme::sidebar_background(cx)
        } else {
            theme::content_background(cx)
        };
        let divider = self.vertical_divider(cx);
        let drag = self.window_drag_area();
        el! {
            .div { relative; flex; flex_shrink_0; w_full; h: TOOLBAR_HEIGHT.px } [
                .div {
                    w: width.px; h_full; flex_shrink_0; flex; items_center;
                    px: MENU_PADDING.px; bg: leading_background;
                } [
                    menu,
                    @if (let Some(drag) = drag) $(drag), @end,
                ],
                .div {
                    flex_1; min_w_0; h_full; flex; items_center;
                    px: SIDEBAR_PADDING.px; gap_1; bg: $(theme::content_background(cx));
                } [
                    .div { flex_1; min_w_0; h_full; flex; items_center; overflow_hidden } [controls],
                    window_controls,
                ],
                ..divider,
            ]
        }
    }

    pub fn workspace(&self, sidebar: AnyElement, content: AnyElement, cx: &App) -> Div {
        let width = self.sidebar_width;
        let divider = self.vertical_divider(cx);
        el! {
            .div { relative; flex; flex_1; min_h_0; w_full } [
                @if (let Some(width) = width)
                    .div { w: width.px; h_full; flex_shrink_0; bg: $(theme::sidebar_background(cx)) } [sidebar],
                @end,
                .div { flex_1; min_w_0; h_full; bg: $(theme::content_background(cx)) } [content],
                ..divider,
            ]
        }
    }

    /// The window-wide bottom bar keeps sidebar context at the far left and
    /// window-level controls at the far right, including when a dock is open.
    pub fn footer(
        &self,
        sidebar_status: AnyElement,
        status: AnyElement,
        controls: AnyElement,
        cx: &App,
    ) -> Div {
        let width = self.sidebar_width;
        let boundary = width.unwrap_or(0.);
        let divider = self.vertical_divider(cx);
        el! {
            .div { relative; flex; w_full; h: FOOTER_HEIGHT.px; flex_shrink_0 } [
                @if (let Some(width) = width)
                    .div { w: width.px; h_full; flex_shrink_0; bg: $(theme::sidebar_background(cx)) } [sidebar_status],
                @end,
                .div {
                    flex_1; min_w_0; h_full; flex; items_center; gap: ITEM_GAP.px;
                    bg: $(theme::content_background(cx)); px: STATUS_PADDING.px;
                } [
                    .div { flex_1; min_w_0; flex; items_center; gap: ITEM_GAP.px } [status],
                    controls,
                ],
                ..divider,
                .div { absolute; left: boundary.px; right_0; top_0; h: DIVIDER_WIDTH.px; bg: $(cx.theme().border) },
            ]
        }
    }
}
