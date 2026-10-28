//! A reusable single-terminal process and surface, independent of host layout or key maps.
mod backend;
mod input;
mod instance;
mod preferences;
mod session;
mod view;

pub use instance::{Shutdown, Status, Terminal};
pub use preferences::{Preferences, PreferencesEditor};

use gpui_kit::{App, Global, actions};

pub const INPUT_CONTEXT: &str = "GdsTerminal";

/// Observable process state/title change. Host navigation is not a terminal event.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    Changed,
}

// Capabilities, not a key map. Applications choose if and where to bind them.
actions!(
    terminal,
    [
        Copy,
        Paste,
        ScrollPageUp,
        ScrollPageDown,
        ScrollTop,
        ScrollBottom
    ]
);

struct Initialized;
impl Global for Initialized {}

/// Install the process-cleanup fallback. No keyboard shortcuts or UI are registered.
pub fn init(cx: &mut App) {
    if cx.has_global::<Initialized>() {
        return;
    }
    cx.set_global(Initialized);
    cx.on_app_quit(|_| async { session::shutdown_all().await })
        .detach();
}
