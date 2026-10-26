use gpui_kit::component::ThemeColor;
use gpui_kit::{Hsla, rgb};

use super::{Error, Mode};

const ROW_HOVER_OPACITY: f32 = 0.055;
const SCROLL_THUMB_OPACITY: f32 = 0.35;
const SCROLL_THUMB_HOVER_OPACITY: f32 = 0.55;
/// How far a danger surface darkens toward its pressed shade.
const DANGER_HOVER_SHADE: f32 = 0.07;
/// The same move, further, for a held (pressed) danger surface.
const DANGER_ACTIVE_SHADE: f32 = 0.12;

#[derive(Clone, Copy, Debug)]
pub(super) struct Palette {
    pub content: Hsla,
    pub sidebar: Hsla,
    pub foreground: Hsla,
    pub sidebar_foreground: Hsla,
    pub muted_foreground: Hsla,
    pub surface: Hsla,
    pub surface_foreground: Hsla,
    pub border: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub selection: Hsla,
    pub selection_foreground: Hsla,
    pub tooltip: Hsla,
    pub tooltip_foreground: Hsla,
    pub link: Hsla,
    pub error: Hsla,
    pub error_foreground: Hsla,
}

impl Palette {
    pub fn builtin(mode: Mode) -> Self {
        match mode {
            Mode::Dark => Self {
                content: rgb(0x1c221f).into(),
                sidebar: rgb(0x171d1a).into(),
                foreground: rgb(0xe2e9e4).into(),
                sidebar_foreground: rgb(0xe2e9e4).into(),
                muted_foreground: rgb(0x98aaa0).into(),
                surface: rgb(0x242d28).into(),
                surface_foreground: rgb(0xe2e9e4).into(),
                border: rgb(0x3c4941).into(),
                primary: rgb(0xadd5bd).into(),
                primary_foreground: rgb(0x173322).into(),
                selection: rgb(0x314c3c).into(),
                selection_foreground: rgb(0xe7f5eb).into(),
                tooltip: rgb(0x242d28).into(),
                tooltip_foreground: rgb(0xe2e9e4).into(),
                link: rgb(0xadd5bd).into(),
                error: rgb(0xf0a6a3).into(),
                error_foreground: rgb(0x521e1d).into(),
            },
            Mode::Light => Self {
                content: rgb(0xf9fbf7).into(),
                sidebar: rgb(0xedf3eb).into(),
                foreground: rgb(0x25352b).into(),
                sidebar_foreground: rgb(0x25352b).into(),
                muted_foreground: rgb(0x64766b).into(),
                surface: rgb(0xf7faf3).into(),
                surface_foreground: rgb(0x25352b).into(),
                border: rgb(0xcbd8cb).into(),
                primary: rgb(0x356e4b).into(),
                primary_foreground: rgb(0xffffff).into(),
                selection: rgb(0xd8e9d8).into(),
                selection_foreground: rgb(0x21452d).into(),
                tooltip: rgb(0xf7faf3).into(),
                tooltip_foreground: rgb(0x25352b).into(),
                link: rgb(0x356e4b).into(),
                error: rgb(0xa33938).into(),
                error_foreground: rgb(0xffffff).into(),
            },
        }
    }

    pub fn apply_to(self, colors: &mut ThemeColor) {
        colors.background = self.content;
        colors.foreground = self.foreground;
        colors.border = self.border;
        colors.input = self.border;
        colors.primary = self.primary;
        colors.primary_foreground = self.primary_foreground;
        colors.primary_hover = self.primary;
        colors.primary_active = self.primary;
        colors.ring = self.primary;
        colors.caret = self.primary;
        colors.selection = self.selection;
        colors.accent = self.selection;
        colors.accent_foreground = self.selection_foreground;
        colors.muted = self.surface;
        colors.muted_foreground = self.muted_foreground;
        colors.secondary = self.surface;
        colors.secondary_foreground = self.surface_foreground;
        colors.secondary_hover = self.selection;
        colors.secondary_active = self.selection;
        colors.button = self.surface;
        colors.button_foreground = self.surface_foreground;
        colors.button_hover = self.selection;
        colors.button_active = self.selection;
        colors.button_primary = self.primary;
        colors.button_primary_foreground = self.primary_foreground;
        colors.button_primary_hover = self.primary;
        colors.button_primary_active = self.primary;
        colors.button_secondary = self.surface;
        colors.button_secondary_foreground = self.surface_foreground;
        colors.button_secondary_hover = self.selection;
        colors.button_secondary_active = self.selection;
        colors.popover = self.tooltip;
        colors.popover_foreground = self.tooltip_foreground;
        colors.group_box = self.surface;
        colors.group_box_foreground = self.surface_foreground;
        colors.accordion = self.surface;
        colors.sidebar = self.sidebar;
        colors.sidebar_foreground = self.sidebar_foreground;
        colors.sidebar_border = self.border;
        colors.sidebar_accent = self.selection;
        colors.sidebar_accent_foreground = self.selection_foreground;
        colors.sidebar_primary = self.primary;
        colors.sidebar_primary_foreground = self.primary_foreground;
        colors.list = self.content;
        colors.list_even = self.content;
        colors.list_head = self.surface;
        colors.list_hover = self.surface;
        colors.list_active = self.selection;
        colors.list_active_border = self.primary;
        colors.table = self.content;
        colors.table_even = self.content;
        colors.table_head = self.surface;
        colors.table_head_foreground = self.muted_foreground;
        colors.table_hover = self.foreground.opacity(ROW_HOVER_OPACITY);
        colors.table_active = self.selection;
        colors.table_active_border = self.primary;
        colors.table_row_border = self.border.opacity(0.0);
        colors.tab = self.sidebar;
        colors.tab_foreground = self.sidebar_foreground;
        colors.tab_active = self.selection;
        colors.tab_active_foreground = self.selection_foreground;
        colors.tab_bar = self.sidebar;
        colors.title_bar = self.sidebar;
        colors.title_bar_border = self.border;
        colors.status_bar = self.sidebar;
        colors.status_bar_border = self.border;
        colors.link = self.link;
        colors.link_hover = self.link;
        colors.link_active = self.link;
        colors.scrollbar = self.content.opacity(0.0);
        colors.scrollbar_thumb = self.foreground.opacity(SCROLL_THUMB_OPACITY);
        colors.scrollbar_thumb_hover = self.foreground.opacity(SCROLL_THUMB_HOVER_OPACITY);
        colors.danger = self.error;
        colors.danger_foreground = self.error_foreground;
        let danger_hover = shaded(self.error, DANGER_HOVER_SHADE);
        let danger_active = shaded(self.error, DANGER_ACTIVE_SHADE);
        colors.danger_hover = danger_hover;
        colors.danger_active = danger_active;
        colors.button_danger = self.error;
        colors.button_danger_foreground = self.error_foreground;
        colors.button_danger_hover = danger_hover;
        colors.button_danger_active = danger_active;
    }
}

/// A darker shade of a filled action color.
///
/// Only lightness moves, so the theme keeps its danger/foreground pair and the
/// paired text stays the label. Darkening is visible for both a light and a
/// dark danger color, and it never moves the surface toward its own text.
fn shaded(color: Hsla, amount: f32) -> Hsla {
    Hsla {
        l: (color.l - amount).clamp(0., 1.),
        ..color
    }
}

/// Parse an RGB color; Qt's optional leading alpha is intentionally discarded.
/// GDS controls opacity independently and keeps text/popover colors opaque.
///
/// # Errors
/// Rejects unsupported color formats or non-hexadecimal digits.
pub(super) fn hex_color(value: &str, field: &str, qt_argb: bool) -> Result<Hsla, Error> {
    let Some(hex) = value.trim().strip_prefix('#') else {
        return Err(Error::invalid(
            field,
            "expected a hexadecimal color starting with #",
        ));
    };
    let number = u32::from_str_radix(hex, 16)
        .map_err(|_| Error::invalid(field, format!("invalid hexadecimal color {value:?}")))?;
    let color = match hex.len() {
        3 => ((number >> 8) * 17) << 16 | (((number >> 4) & 15) * 17) << 8 | (number & 15) * 17,
        6 => number,
        8 if qt_argb => number & 0x00ff_ffff,
        _ => {
            return Err(Error::invalid(
                field,
                "expected #RGB, #RRGGBB or Qt #AARRGGBB",
            ));
        }
    };
    Ok(rgb(color).into())
}
