//! Suite-wide appearance settings and read-only desktop palette adapters.

mod dms;
mod hot_reload;
mod observer;
mod palette;
mod qt;
#[cfg(test)]
mod tests;

pub use hot_reload::HotReload;
pub use observer::{Change, Observer};

use std::{fs, path::PathBuf};

use directories::BaseDirs;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Global, Hsla, WindowAppearance};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use palette::Palette;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Builtin,
    Qt6ct,
    Qt5ct,
    #[default]
    Dms,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Light,
    #[default]
    Dark,
}

impl From<Mode> for ThemeMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Light => Self::Light,
            Mode::Dark => Self::Dark,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ModePreference {
    /// Resolve a platform-backed preference without changing the stored choice.
    pub fn resolve(self, appearance: WindowAppearance) -> Mode {
        match self {
            Self::Light => Mode::Light,
            Self::Dark => Mode::Dark,
            Self::System => match appearance {
                WindowAppearance::Light | WindowAppearance::VibrantLight => Mode::Light,
                WindowAppearance::Dark | WindowAppearance::VibrantDark => Mode::Dark,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Appearance {
    pub source: Source,
    /// System follows the selected source; Light/Dark remain explicit overrides.
    pub mode: ModePreference,
    pub content_opacity: f32,
    pub sidebar_opacity: f32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            source: Source::default(),
            mode: ModePreference::default(),
            content_opacity: crate::design::CONTENT_OPACITY,
            sidebar_opacity: crate::design::SIDEBAR_OPACITY,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    pub source: Source,
    pub mode: Mode,
    /// The palette actually read, not just the qtct configuration pointer.
    pub source_path: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("could not determine the user's XDG directories")]
    NoUserDirectories,
    #[error("could not read theme file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid theme JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid theme INI in {path}: {source}")]
    Ini {
        path: PathBuf,
        #[source]
        source: ini::ParseError,
    },
    #[error("theme field {field}: {detail}")]
    Invalid { field: String, detail: String },
    #[error("custom_palette is not enabled in {0}")]
    CustomPaletteDisabled(PathBuf),
    #[error("{name} must be finite and between 0 and 1, got {value}")]
    InvalidOpacity { name: &'static str, value: f32 },
    #[error("apply an appearance before updating its opacity")]
    MaterialNotInitialized,
}

impl Error {
    fn invalid(field: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Invalid {
            field: field.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Copy)]
struct Material {
    content: Hsla,
    sidebar: Hsla,
    content_opacity: f32,
    sidebar_opacity: f32,
}
impl Global for Material {}

/// Read the selected source and apply one fully parsed palette to GPUI Kit.
///
/// Call after `gpui_kit::init`. This is a synchronous startup/manual-refresh
/// operation; [`HotReload`] owns live refreshes and reuses the same loading and
/// installation path. It does not modify desktop files.
/// Controls and text remain opaque. Root and data-table surfaces are cleared;
/// application-owned panels draw their material once using the helpers below.
///
/// # Errors
/// Returns an error for invalid opacity, unavailable source files, disabled Qt
/// custom palettes, or unsupported/malformed palette data. The previous valid
/// theme and material settings are retained on every such failure.
pub fn apply(appearance: &Appearance, cx: &mut App) -> Result<Applied, Error> {
    appearance.prepare(cx.window_appearance())?.install(cx)
}

/// A validated desktop palette that can be read away from the UI thread.
/// Applications can compare snapshots before installing external changes.
#[derive(Clone, Debug, PartialEq)]
pub struct Prepared {
    appearance: Appearance,
    loaded: Loaded,
}

impl Prepared {
    /// Files that can change this prepared appearance, including palette indirection.
    pub fn source_paths(&self) -> Vec<PathBuf> {
        let mut paths = self.appearance.source_paths();
        paths.extend(self.loaded.source_path.clone());
        paths.sort();
        paths.dedup();
        paths
    }
    /// Install the prepared palette without reading source files again.
    ///
    /// # Errors
    /// Reports invalid surface opacity; the previous palette remains installed.
    pub fn install(self, cx: &mut App) -> Result<Applied, Error> {
        apply_loaded(&self.appearance, self.loaded, cx)
    }
}

/// Install one fully parsed palette. Shared by the synchronous [`apply`] path
/// and background refreshes, so every entry point writes the same state.
///
/// # Errors
/// Returns an error for invalid opacity. Neither opacity is changed and the
/// previous theme stays applied if validation fails.
fn apply_loaded(appearance: &Appearance, loaded: Loaded, cx: &mut App) -> Result<Applied, Error> {
    validate_opacity("content_opacity", appearance.content_opacity)?;
    validate_opacity("sidebar_opacity", appearance.sidebar_opacity)?;
    let Loaded {
        palette,
        mode,
        source_path,
    } = loaded;

    // A mode change reloads Kit's registered palette. Apply custom colors only
    // afterwards; Theme::update reconciles its colors/tokens/Base projection.
    Theme::change(ThemeMode::from(mode), None, cx);
    Theme::update(cx, |theme| palette.apply_to(&mut theme.colors));
    cx.set_global(Material {
        content: palette.content,
        sidebar: palette.sidebar,
        content_opacity: appearance.content_opacity,
        sidebar_opacity: appearance.sidebar_opacity,
    });
    // Component's Root paints tokens.background behind all panels. Clear both
    // representations together so token reconciliation cannot restore an opaque
    // underlay; the material helpers retain the real RGB independently.
    Theme::update(cx, |theme| {
        let mut clear = palette.content;
        clear.a = 0.0;
        theme.colors.background = clear;
        theme.tokens.background = clear.into();
        // Data tables share the surrounding content material, not a second underlay.
        theme.colors.table = clear;
        theme.tokens.table = clear.into();
        theme.colors.table_even = clear;
        theme.tokens.table_even = clear.into();
        theme.colors.table_head = clear;
        theme.tokens.table_head = clear.into();
        theme.colors.table_row_border = clear;
        theme.tokens.table_row_border = clear.into();
        theme.colors.scrollbar = clear;
        theme.tokens.scrollbar = clear.into();
    });
    // Kit reloads the registered theme on a mode change, and a theme file may
    // name its own fonts. Re-assert the user's typography so switching theme,
    // mode or opacity never resets the font family or size.
    crate::typography::reapply(cx);
    // Every theme application re-aligns the shared toast column, so switching
    // themes cannot move it or leave it under the toolbar.
    crate::notifications::install(cx);
    Ok(Applied {
        source: appearance.source,
        mode,
        source_path,
    })
}

/// Update only the existing surface opacity, without reloading a palette.
///
/// This is the slider update path: it does not read source files, change the
/// selected mode, or touch Kit's text, selection or transparent-root colors.
///
/// # Errors
/// Returns an error for non-finite/out-of-range alpha or when no appearance has
/// been applied yet. Neither opacity is changed if validation fails.
pub fn update_opacity(content: f32, sidebar: f32, cx: &mut App) -> Result<(), Error> {
    validate_opacity("content_opacity", content)?;
    validate_opacity("sidebar_opacity", sidebar)?;
    if !cx.has_global::<Material>() {
        return Err(Error::MaterialNotInitialized);
    }
    let material = cx.global_mut::<Material>();
    material.content_opacity = content;
    material.sidebar_opacity = sidebar;
    cx.refresh_windows();
    Ok(())
}

/// Background for the application-owned content surface, not text or controls.
/// The window/root must permit transparency for alpha to reach the desktop.
pub fn content_background(cx: &App) -> Hsla {
    let Some(material) = cx.try_global::<Material>() else {
        return Theme::global(cx).background;
    };
    let mut color = material.content;
    color.a = material.content_opacity;
    color
}

/// Background for the application-owned sidebar and toolbar surfaces.
pub fn sidebar_background(cx: &App) -> Hsla {
    let Some(material) = cx.try_global::<Material>() else {
        return Theme::global(cx).sidebar;
    };
    let mut color = material.sidebar;
    color.a = material.sidebar_opacity;
    color
}

/// # Errors
/// Rejects non-finite alpha and values outside the supported range.
fn validate_opacity(name: &'static str, value: f32) -> Result<(), Error> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::InvalidOpacity { name, value })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Loaded {
    palette: Palette,
    mode: Mode,
    source_path: Option<PathBuf>,
}

impl Appearance {
    /// Desktop source pointers; a prepared palette adds its resolved palette file.
    pub fn source_paths(&self) -> Vec<PathBuf> {
        match self.source {
            Source::Builtin => Vec::new(),
            Source::Dms => dms_colors_path().into_iter().collect(),
            Source::Qt5ct | Source::Qt6ct => {
                let name = if self.source == Source::Qt6ct {
                    "qt6ct"
                } else {
                    "qt5ct"
                };
                BaseDirs::new()
                    .map(|dirs| dirs.config_dir().join(name).join(format!("{name}.conf")))
                    .into_iter()
                    .collect()
            }
        }
    }
    /// Read and validate one complete palette for a known platform appearance.
    ///
    /// # Errors
    /// Reports invalid opacity, unavailable source files or malformed palette data.
    pub fn prepare(&self, window_appearance: WindowAppearance) -> Result<Prepared, Error> {
        validate_opacity("content_opacity", self.content_opacity)?;
        validate_opacity("sidebar_opacity", self.sidebar_opacity)?;
        Ok(Prepared {
            appearance: self.clone(),
            loaded: self.load_for(window_appearance)?,
        })
    }

    /// Use a known local palette and safe opacity after a source-load failure.
    /// The user's mode preference is retained, without modifying saved settings.
    pub fn builtin_fallback(&self) -> Self {
        Self {
            source: Source::Builtin,
            mode: self.mode,
            ..Self::default()
        }
    }

    /// Resolve the selected source against a known platform appearance.
    ///
    /// # Errors
    /// Propagates source discovery, file access and palette decoding failures.
    fn load_for(&self, window_appearance: WindowAppearance) -> Result<Loaded, Error> {
        match self.source {
            Source::Builtin => {
                let mode = self.mode.resolve(window_appearance);
                Ok(Loaded {
                    palette: Palette::builtin(mode),
                    mode,
                    source_path: None,
                })
            }
            Source::Dms => {
                let path = dms_colors_path().ok_or(Error::NoUserDirectories)?;
                let parsed = dms::parse(&read(&path)?, &path, self.mode)?;
                Ok(Loaded {
                    palette: parsed.palette,
                    mode: parsed.mode,
                    source_path: Some(path),
                })
            }
            Source::Qt6ct | Source::Qt5ct => {
                let dirs = BaseDirs::new().ok_or(Error::NoUserDirectories)?;
                let name = if self.source == Source::Qt6ct {
                    "qt6ct"
                } else {
                    "qt5ct"
                };
                let config = dirs.config_dir().join(name).join(format!("{name}.conf"));
                let palette_path = qt::palette_path(&read(&config)?, &config, dirs.home_dir())?;
                let mode = self.mode.resolve(window_appearance);
                let palette = qt::parse(&read(&palette_path)?, &palette_path, mode)?;
                Ok(Loaded {
                    palette,
                    mode,
                    source_path: Some(palette_path),
                })
            }
        }
    }
}

/// File DankMaterialShell exports its current color scheme to.
fn dms_colors_path() -> Option<PathBuf> {
    BaseDirs::new().map(|dirs| dirs.cache_dir().join("DankMaterialShell/dms-colors.json"))
}

/// # Errors
/// Reports inaccessible files and non-UTF-8 theme content with their path.
fn read(path: &std::path::Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })
}
