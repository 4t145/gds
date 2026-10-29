//! Default GDS visual language, shared by every application and its reset actions.
//!
//! Dimensions are logical pixels; platform display scaling still applies. These
//! are defaults, not persisted suite-wide preferences. Application settings may
//! override typography and surface opacity independently.

use gpui_kit::WindowBackgroundAppearance;

pub const TOOLBAR_HEIGHT: f32 = 52.0;
pub const FOOTER_HEIGHT: f32 = 36.0;
pub const SIDEBAR_WIDTH: f32 = 222.0;
pub const COLLAPSED_SIDEBAR_WIDTH: f32 = 58.0;
pub const SIDEBAR_PADDING: f32 = 10.0;
/// Section labels sit inside the sidebar's padded row, aligned across sections.
pub const SIDEBAR_SECTION_PADDING: f32 = 6.0;
/// Standard horizontal inset for content rows and their headers.
pub const CONTENT_PADDING: f32 = 12.0;
/// Terminal window titles and the PTY grid share the host content-row edge.
pub const TERMINAL_CONTENT_INSET: f32 = CONTENT_PADDING;
pub const SIDEBAR_ITEM_HEIGHT: f32 = 38.0;
pub const ICON_BUTTON_SIZE: f32 = 32.0;
/// Dock actions retain a 4px breathing space above and below the standard target.
pub const TERMINAL_HEADER_HEIGHT: f32 = ICON_BUTTON_SIZE + 8.0;

pub const ICON_SIZE: f32 = 16.0;
pub const SIDEBAR_ICON_SIZE: f32 = 18.0;
pub const SMALL_ICON_SIZE: f32 = 12.0;

/// The rem base, distinct from the 14px text used by ordinary UI controls.
pub const FONT_SIZE: f32 = 16.0;
pub const UI_TEXT_SCALE: f32 = 0.875;

/// Opacity applies only to surface backgrounds; text and icons stay opaque.
pub const CONTENT_OPACITY: f32 = 0.85;
pub const SIDEBAR_OPACITY: f32 = 0.5;
/// An alpha-capable window lets per-surface opacity changes apply immediately.
pub const WINDOW_BACKGROUND: WindowBackgroundAppearance = WindowBackgroundAppearance::Transparent;
