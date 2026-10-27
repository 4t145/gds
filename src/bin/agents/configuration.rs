//! The selected configuration store is shared by all frontend features.

use std::path::PathBuf;

use bas::config::FrontendStore as Store;
use gpui_kit::{App, Global};

struct Configuration(Store);
impl Global for Configuration {}

pub(crate) fn init(settings_file: Option<PathBuf>, cx: &mut App) {
    let settings_file = settings_file.unwrap_or_else(|| {
        bas::config::paths::config_root()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("gds/agents/settings.json")
    });
    cx.set_global(Configuration(Store::new(Some(settings_file))));
}

pub(crate) fn store(cx: &App) -> Store {
    cx.global::<Configuration>().0.clone()
}

#[cfg(all(test, feature = "gui-tests"))]
mod tests;
