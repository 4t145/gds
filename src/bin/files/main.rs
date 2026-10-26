mod app;
mod directory;
mod file_clipboard;
mod navigation;
mod operations;
mod picker;
mod places;
mod sorting;

use std::process::ExitCode;

use anyhow::Context as _;
use directories::BaseDirs;
use fluent_bundle::FluentArgs;
use gds::{actions, i18n, settings, theme, time, typography};
use gpui_kit::*;

const APP_ID: &str = "io.github.fourt145.GdsFiles";
const PICKER_APP_ID: &str = "io.github.fourt145.GdsFiles.Picker";
const PICKER_WIDTH: f32 = 1100.;
const PICKER_HEIGHT: f32 = 740.;
const INITIAL_WIDTH: f32 = 1280.;
const INITIAL_HEIGHT: f32 = 820.;
/// The window has to hold the sidebar column, the file content and the details
/// panel side by side; below this they would collide in the toolbar and the
/// content area. The dialog and column layouts do not shrink past these.
const MIN_WIDTH: f32 = 1000.;
const MIN_HEIGHT: f32 = 640.;

const HELP: &str = "\
gds-files [DIRECTORY]
gds-files --pick-file [DIRECTORY] [--multiple] [--filter [LABEL:]GLOB[,GLOB...]]... [--read-only]
gds-files --pick-directory [DIRECTORY] [--multiple]

A native GPUI file browser for Linux / Niri.
Run inside a graphical Wayland or X11 session.

Arguments:
  DIRECTORY              Directory to open (default: home). Use -- before a
                         directory name that starts with '-'.

Options:
  --pick-file            Pick files, print absolute paths as a single
                         JSON line on stdout and exit.
  --pick-directory       Pick a folder (or the current folder) and print JSON.
  --multiple             Allow selecting multiple files or folders in picker mode.
  --filter [LABEL:]GLOB  Restrict the picker to matching file names. Repeatable;
                         every value becomes a switchable filter group, e.g.
                         --filter 'Text:*.txt,*.md' --filter '*.rs'
                         Only the basename is matched and folders stay
                         navigable. Without --filter, every file is listed.
  --read-only            Open the picked file read-only (only with --pick-file).
  -h, --help             Print this help.
  -V, --version          Print the version.

Exit status with --pick-file: 0 file selected, 1 cancelled, 2 output failed.
";

/// # Errors
/// Reports invalid arguments or unavailable user/current directories.
fn main() -> anyhow::Result<ExitCode> {
    let arguments = match picker::parse(std::env::args_os().skip(1)) {
        Ok(arguments) => arguments,
        Err(error) => {
            eprintln!("gds-files: {error}");
            eprintln!("Try 'gds-files --help' for usage.");
            picker::mark_failed();
            return Ok(ExitCode::from(picker::exit_code()));
        }
    };
    if arguments.help {
        print!("{HELP}");
        return Ok(ExitCode::SUCCESS);
    }
    if arguments.version {
        println!("gds-files {}", env!("CARGO_PKG_VERSION"));
        return Ok(ExitCode::SUCCESS);
    }
    let picking = arguments.picker.is_some();
    match launch(arguments) {
        Ok(()) if picking => Ok(ExitCode::from(picker::exit_code())),
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(error) => {
            eprintln!("gds-files: {error:#}");
            if picking {
                picker::mark_failed();
                return Ok(ExitCode::from(picker::exit_code()));
            }
            Ok(ExitCode::FAILURE)
        }
    }
}

/// # Errors
/// Reports unavailable user/current directories.
fn launch(arguments: picker::Arguments) -> anyhow::Result<()> {
    let home = BaseDirs::new()
        .context("cannot determine the home directory")?
        .home_dir()
        .to_path_buf();
    let start = match arguments.directory {
        Some(path) if path.is_absolute() => path,
        Some(path) => std::env::current_dir()?.join(path),
        None => home.clone(),
    };
    let picker_options = arguments.picker;
    let picking = picker_options.is_some();
    let (preferences, migration_error, mut startup_error) = match settings::load_startup() {
        Ok(loaded) => (loaded.preferences, loaded.migration, None),
        Err(error) => (
            settings::Preferences::default(),
            None,
            Some(error.to_string()),
        ),
    };
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_app_identity(APP_ID, "GDS Files");
            if let Err(error) = i18n::install(
                preferences.language.unwrap_or_else(i18n::Language::detect),
                cx,
            ) {
                eprintln!("Cannot load application messages: {error}");
                if picking {
                    picker::mark_failed();
                }
                cx.quit();
                return;
            }
            time::apply(preferences.time_format, cx);
            if let Err(error) = theme::apply(&preferences.appearance, cx) {
                startup_error = Some(match startup_error.take() {
                    Some(previous) => format!("{previous}\n{error}"),
                    None => error.to_string(),
                });
                let fallback = preferences.appearance.builtin_fallback();
                if let Err(error) = theme::apply(&fallback, cx) {
                    eprintln!("Cannot initialize the built-in theme: {error}");
                    if picking {
                        picker::mark_failed();
                    }
                    cx.quit();
                    return;
                }
            }
            match typography::install(&preferences.font, cx) {
                Ok(applied) => {
                    if let Some(unavailable) = applied.unavailable {
                        eprintln!(
                            "Font family {unavailable:?} is not installed; using the system font."
                        );
                    }
                }
                Err(error) => {
                    startup_error = Some(match startup_error.take() {
                        Some(previous) => format!("{previous}\n{error}"),
                        None => error.to_string(),
                    });
                    // An unusable stored size must not stop the window from
                    // opening, and the stored configuration is left untouched.
                    if let Err(fallback) =
                        typography::install(&typography::Preferences::default(), cx)
                    {
                        eprintln!("Cannot initialize the default font: {fallback}");
                    }
                }
            }
            actions::register(cx);
            if let Some(error) = migration_error {
                let mut arguments = FluentArgs::new();
                arguments.set("error", error.to_string());
                let message = i18n::format("settings-migrate-failed", Some(&arguments), cx);
                startup_error = Some(match startup_error.take() {
                    Some(previous) => format!("{previous}\n{message}"),
                    None => message.to_string(),
                });
            }
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let dimensions = if picking {
                size(px(PICKER_WIDTH), px(PICKER_HEIGHT))
            } else {
                size(px(INITIAL_WIDTH), px(INITIAL_HEIGHT))
            };
            let bounds = Bounds::centered(None, dimensions, cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(MIN_WIDTH), px(MIN_HEIGHT))),
                titlebar: None,
                window_background: WindowBackgroundAppearance::Transparent,
                app_id: Some(if picking { PICKER_APP_ID } else { APP_ID }.to_owned()),
                kind: if picking {
                    WindowKind::Dialog
                } else {
                    WindowKind::Normal
                },
                ..Default::default()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, move |window, cx| {
                window.set_window_title(i18n::text("app-title", cx).as_ref());
                let launch = app::LaunchOptions {
                    start,
                    home,
                    preferences,
                    startup_error,
                    picker: picker_options,
                };
                cx.new(|cx| app::Browser::new(launch, window, cx))
            }) {
                eprintln!("Cannot open the Files window: {error}");
                if picking {
                    picker::mark_failed();
                }
                cx.quit();
            }
        });
    Ok(())
}
