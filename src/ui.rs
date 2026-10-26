//! Shared window geometry, controls and shortcut presentation.

use std::{rc::Rc, time::Duration};

use gds_uim::el;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable};
use gpui_kit::{prelude::*, *};

use crate::{actions::Command, theme, typography};

pub mod dialog;
pub mod keytips;
pub use dialog::DialogActions;

pub const SIDEBAR_WIDTH: f32 = 222.0;
pub const COLLAPSED_SIDEBAR_WIDTH: f32 = 58.0;
pub const TOOLBAR_HEIGHT: f32 = 52.0;
pub const FOOTER_HEIGHT: f32 = 36.0;
pub const SIDEBAR_PADDING: f32 = 10.0;
pub const SIDEBAR_ITEM_HEIGHT: f32 = 38.0;
pub const SIDEBAR_ICON_SIZE: f32 = 18.0;
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(400);
pub const ICON_BUTTON_SIZE: f32 = 32.0;
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
}

/// An action together with the key context whose binding applies to it.
#[derive(Clone)]
pub struct Shortcut {
    action: Rc<dyn Action>,
    context: Option<&'static str>,
    prefix: Option<PrefixTarget>,
}

/// A key that only runs behind the `Ctrl+Tab` prefix.
///
/// Its binding lives in [`TAB_PREFIX_CONTEXT`](crate::actions::TAB_PREFIX_CONTEXT),
/// so presenting it as a plain Ctrl key would advertise a press that does
/// something else. The hint layer instead shows the prefix entry followed by the
/// target (`Tab+3`) while the plain layer is up, and only the target (`3`) while
/// the prefix is armed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrefixTarget {
    entry: Command,
}

impl PrefixTarget {
    /// The command that arms the prefix (`Ctrl+Tab`).
    pub fn entry(self) -> Command {
        self.entry
    }
}

impl Shortcut {
    pub fn new(action: Box<dyn Action>, context: Option<&'static str>) -> Self {
        Self {
            action: Rc::from(action),
            context,
            prefix: None,
        }
    }

    pub fn context(&self) -> Option<&'static str> {
        self.context
    }

    pub fn action(&self) -> &dyn Action {
        &*self.action
    }

    /// Mark this as a page key that only runs behind the prefix `entry` arms.
    pub fn via_prefix(mut self, entry: Command) -> Self {
        self.prefix = Some(PrefixTarget { entry });
        self
    }

    /// The prefix this key belongs behind, for a page key; `None` for an
    /// ordinary shortcut that runs from the plain layer itself.
    pub fn prefix(&self) -> Option<PrefixTarget> {
        self.prefix
    }

    /// Whether both shortcuts name the same action in the same scope, so a
    /// cached presentation does not have to be rebuilt.
    pub fn same_scope(&self, other: &Self) -> bool {
        self.context == other.context
            && self.prefix == other.prefix
            && self.action.partial_eq(&*other.action)
    }

    /// The highest-precedence binding currently in effect for this action.
    pub fn binding(&self, window: &Window) -> Option<KeyBinding> {
        let context = self
            .context
            .map(|value| KeyContext::parse(value).expect("bundled action contexts are valid"))
            .unwrap_or_default();
        window.highest_precedence_binding_for_action_in_context(&*self.action, context)
    }
}

impl From<Command> for Shortcut {
    fn from(command: Command) -> Self {
        Self::new(command.action(), command.context())
    }
}

/// The modifier group a press reveals hint labels for.
///
/// The two groups never mix. Ctrl+Alt (what AltGr reports) selects neither, so
/// such a press shows no labels instead of lighting the other group's keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeldModifier {
    Ctrl,
    Alt,
}

impl HeldModifier {
    /// The group the current modifier state asks for, if any.
    pub fn from_modifiers(modifiers: Modifiers) -> Option<Self> {
        if modifiers.platform {
            return None;
        }
        match (modifiers.control, modifiers.alt) {
            (true, false) => Some(Self::Ctrl),
            (false, true) => Some(Self::Alt),
            _ => None,
        }
    }

    /// Whether a stroke carries this group's modifier.
    pub fn matches(self, modifiers: Modifiers) -> bool {
        match self {
            Self::Ctrl => modifiers.control,
            Self::Alt => modifiers.alt,
        }
    }
}

#[derive(IntoElement)]
pub struct ShortcutBadge {
    source: BadgeSource,
}

/// Where a badge's keys come from: a control's own shortcut, resolved in its
/// declared scope, or a binding already resolved for the current context.
enum BadgeSource {
    Shortcut(Shortcut),
    Binding(KeyBinding),
}

impl ShortcutBadge {
    pub fn new(command: Command) -> Self {
        Self::bound(command.into())
    }

    /// A badge for any action, resolved through its own key context.
    pub fn bound(shortcut: Shortcut) -> Self {
        Self {
            source: BadgeSource::Shortcut(shortcut),
        }
    }

    /// A badge for a binding that is already known to apply right now.
    pub fn binding(binding: KeyBinding) -> Self {
        Self {
            source: BadgeSource::Binding(binding),
        }
    }

    fn labels(&self, window: &Window) -> Vec<SharedString> {
        let binding = match &self.source {
            BadgeSource::Shortcut(shortcut) => shortcut.binding(window),
            BadgeSource::Binding(binding) => Some(binding.clone()),
        };
        key_labels(binding.as_ref())
    }

    /// The whole combination as one compact label, such as `R` or `Shift+Tab`,
    /// with the modifier the user is already holding left out because the press
    /// itself shows it.
    ///
    /// It is spelled from [`key_labels`], the same names the keycaps use, so the
    /// two presentations cannot drift apart.
    pub(crate) fn path_label(bindings: &[KeyBinding], held: HeldModifier) -> Option<SharedString> {
        let labels = bindings
            .iter()
            .flat_map(|binding| key_labels_omitting(Some(binding), Some(held)))
            .collect::<Vec<_>>();
        if labels.is_empty() {
            return None;
        }
        let parts = labels.iter().map(AsRef::<str>::as_ref).collect::<Vec<_>>();
        Some(parts.join("+").into())
    }
}

fn key_labels(binding: Option<&KeyBinding>) -> Vec<SharedString> {
    key_labels_omitting(binding, None)
}

/// The key names of a binding's first stroke, minus the prefix of `held`.
fn key_labels_omitting(
    binding: Option<&KeyBinding>,
    held: Option<HeldModifier>,
) -> Vec<SharedString> {
    let mut labels = Vec::new();
    if let Some(stroke) = binding
        .and_then(|binding| binding.keystrokes().first())
        .map(AsKeystroke::as_keystroke)
    {
        if stroke.modifiers.control && held != Some(HeldModifier::Ctrl) {
            labels.push("Ctrl".into());
        }
        if stroke.modifiers.alt && held != Some(HeldModifier::Alt) {
            labels.push("Alt".into());
        }
        if stroke.modifiers.shift {
            labels.push("Shift".into());
        }
        if stroke.modifiers.platform {
            labels.push("Super".into());
        }
        labels.push(match stroke.key.as_str() {
            "left" => "←".into(),
            "right" => "→".into(),
            "up" => "↑".into(),
            "down" => "↓".into(),
            "enter" => "Enter".into(),
            "escape" => "Esc".into(),
            "tab" => "Tab".into(),
            key => key.to_uppercase().into(),
        });
    }
    labels
}

impl RenderOnce for ShortcutBadge {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let keys = self.labels(window).into_iter().map(Keycap::new);
        el! { .div { flex; items_center; gap: KEY_GAP.px } [..keys] }
    }
}

#[derive(IntoElement)]
struct Keycap {
    label: SharedString,
}

impl Keycap {
    fn new(label: SharedString) -> Self {
        Self { label }
    }
}

impl RenderOnce for Keycap {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = self.label;
        el! {
            .div {
                flex; items_center; justify_center;
                h: KEY_HEIGHT.px; min_w: KEY_HEIGHT.px; px: KEY_PADDING.px;
                rounded: KEY_GAP.px; border_1;
                border_color: $cx.theme().border;
                bg: $cx.theme().secondary; text_color: $cx.theme().muted_foreground;
                text_size: $(typography::scaled(KEY_TEXT_SIZE, cx));
            } [label]
        }
    }
}

#[derive(Clone)]
struct Hint {
    label: SharedString,
    shortcut: Option<Shortcut>,
}

impl Hint {
    fn command(label: SharedString, command: Option<Command>) -> Self {
        Self {
            label,
            shortcut: command.map(Shortcut::from),
        }
    }

    fn shortcut(label: SharedString, shortcut: Shortcut) -> Self {
        Self {
            label,
            shortcut: Some(shortcut),
        }
    }

    fn builder(self) -> impl Fn(&mut Window, &mut App) -> AnyView {
        move |window, cx| {
            let hint = self.clone();
            Tooltip::element(move |_, _| hint.content()).build(window, cx)
        }
    }

    fn content(&self) -> Div {
        let badge = self.shortcut.clone().map(ShortcutBadge::bound);
        el! {
            .div { flex; items_center; gap: HINT_GAP.px } [$(self.label.clone()), ..badge]
        }
    }
}

/// A tooltip holding a control's short label and the key that runs it.
///
/// The badge reads the highest-precedence binding like every other presentation
/// of a command, so a control whose `Command` it names cannot show another key
/// than the one a press runs.
pub fn command_tooltip(
    label: impl Into<SharedString>,
    command: Command,
) -> impl Fn(&mut Window, &mut App) -> AnyView {
    Hint::command(label.into(), Some(command)).builder()
}

/// A tooltip holding a control's short label and an already-resolved binding.
///
/// It serves controls whose key is not one of the workspace [`Command`]s, such
/// as the picker's own decisions, while keeping the same presentation as every
/// other keyed control.
pub fn shortcut_tooltip(
    label: impl Into<SharedString>,
    shortcut: Shortcut,
) -> impl Fn(&mut Window, &mut App) -> AnyView {
    Hint::shortcut(label.into(), shortcut).builder()
}

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    button: Button,
    label: SharedString,
    command: Option<Command>,
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
        self.button = self.button.icon(Icon::new(icon));
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self.button = self.button.accessibility_label(self.label.clone());
        self
    }

    pub fn command(mut self, command: Command) -> Self {
        self.command = Some(command);
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
        let shortcut = shortcut.or_else(|| command.map(Shortcut::from));
        // A disabled control cannot run its action, so it never advertises a key.
        let hint = match &shortcut {
            Some(shortcut) if !disabled => {
                Some(keytips::anchor(window, cx, id.clone(), shortcut.clone()))
            }
            _ => None,
        };
        let tooltip = match shortcut {
            Some(shortcut) => Hint::shortcut(label, shortcut),
            None => Hint::command(label, command),
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
    command: Option<Command>,
    shortcut: Option<Shortcut>,
    disabled: bool,
}

impl NavItem {
    pub fn new(id: impl Into<ElementId>, icon: IconName, label: impl Into<SharedString>) -> Self {
        let id = id.into();
        let label = label.into();
        let button = el! { (~id).Button::new { ghost; accessibility_label: ~label } };
        Self {
            id,
            button,
            icon,
            label,
            compact: false,
            command: None,
            shortcut: None,
            disabled: false,
        }
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
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
    pub fn command(mut self, command: Command) -> Self {
        self.command = Some(command);
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
        } = self;
        let content = el! {
            .div {
                flex; size_full; min_w_0; items_center; gap: ITEM_GAP.px;
                @if (compact) justify_center; @end;
            } [
                icon.Icon::new { size: SIDEBAR_ICON_SIZE.px },
                @if (!compact) .div { min_w_0; text_ellipsis } [~label], @end,
            ]
        };
        let button = el! {
            button { w_full; h: SIDEBAR_ITEM_HEIGHT.px; px: ITEM_GAP.px; py_0; rounded: ITEM_RADIUS.px } [content]
        };
        let shortcut = shortcut.or_else(|| command.map(Shortcut::from));
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
        let tooltip = match shortcut {
            Some(shortcut) => Hint::shortcut(label.clone(), shortcut),
            None => Hint::command(label.clone(), command),
        }
        .builder();
        el! {
            .div {
                # id; w_full; h: SIDEBAR_ITEM_HEIGHT.px; flex_shrink_0;
                tooltip_show_delay: TOOLTIP_DELAY; tooltip: tooltip;
            } [
                button,
                @if (let Some(hint) = hint) $(hint), @end,
            ]
        }
    }
}

/// A row's own icon, marked with a corner badge when the row is pinned.
///
/// The icon keeps its identity and its shared size whether or not the row is
/// pinned: replacing it with a pin would drop what the row points at, and
/// resizing it would shift the row's label between the two states.
pub fn pinned_icon(icon: IconName, pinned: bool, cx: &App) -> AnyElement {
    el! {
        .div { relative; flex_shrink_0; size: SIDEBAR_ICON_SIZE.px } [
            icon.Icon::new { size: SIDEBAR_ICON_SIZE.px },
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
struct WindowDragArea;

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
    sidebar_width: f32,
}

impl Frame {
    pub fn new(sidebar_width: f32) -> Self {
        Self { sidebar_width }
    }

    fn vertical_divider(&self, cx: &App) -> Div {
        let boundary = self.sidebar_width;
        el! {
            .div { absolute; left: boundary.px; top_0; bottom_0; w: DIVIDER_WIDTH.px; bg: $(cx.theme().border) }
        }
    }

    /// The toolbar's empty stretch after its leading control, when the column
    /// is wide enough to leave one.
    ///
    /// The leading control is an [`ICON_BUTTON_SIZE`] control in both of the
    /// toolbar's forms (the menu and the secondary page's way back), so the
    /// space beside it is the column width minus the control and its padding.
    fn window_drag_area(&self) -> Option<WindowDragArea> {
        let free = self.sidebar_width - 2. * MENU_PADDING - ICON_BUTTON_SIZE;
        (free >= MIN_DRAG_WIDTH).then_some(WindowDragArea)
    }

    /// The window's top row: the menu column, the page controls, and the
    /// column divider the workspace and footer draw at the same coordinate.
    ///
    /// The empty stretch beside the menu moves the window; the menu and the
    /// controls stay ordinary click targets.
    pub fn toolbar(&self, menu: AnyElement, controls: AnyElement, cx: &App) -> Div {
        let width = self.sidebar_width;
        let divider = self.vertical_divider(cx);
        let drag = self.window_drag_area();
        el! {
            .div { relative; flex; flex_shrink_0; w_full; h: TOOLBAR_HEIGHT.px } [
                .div {
                    w: width.px; h_full; flex_shrink_0; flex; items_center;
                    px: MENU_PADDING.px; bg: $(theme::sidebar_background(cx));
                } [
                    menu,
                    @if (let Some(drag) = drag) $(drag), @end,
                ],
                .div {
                    flex_1; min_w_0; h_full; flex; items_center;
                    px: SIDEBAR_PADDING.px; bg: $(theme::content_background(cx));
                } [controls],
                divider,
            ]
        }
    }

    pub fn workspace(&self, sidebar: AnyElement, content: AnyElement, cx: &App) -> Div {
        let width = self.sidebar_width;
        let divider = self.vertical_divider(cx);
        el! {
            .div { relative; flex; flex_1; min_h_0; w_full } [
                .div { w: width.px; h_full; flex_shrink_0; bg: $(theme::sidebar_background(cx)) } [sidebar],
                .div { flex_1; min_w_0; h_full; bg: $(theme::content_background(cx)) } [content],
                divider,
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
        let divider = self.vertical_divider(cx);
        el! {
            .div { relative; flex; w_full; h: FOOTER_HEIGHT.px; flex_shrink_0 } [
                .div { w: width.px; h_full; flex_shrink_0; bg: $(theme::sidebar_background(cx)) } [sidebar_status],
                .div {
                    flex_1; min_w_0; h_full; flex; items_center; gap: ITEM_GAP.px;
                    bg: $(theme::content_background(cx)); px: STATUS_PADDING.px;
                } [
                    .div { flex_1; min_w_0; flex; items_center; gap: ITEM_GAP.px } [status],
                    controls,
                ],
                divider,
                .div { absolute; left: width.px; right_0; top_0; h: DIVIDER_WIDTH.px; bg: $(cx.theme().border) },
            ]
        }
    }
}
