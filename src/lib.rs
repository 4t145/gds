//! Shared foundation for the GDS desktop applications.

extern crate self as gds;
pub use gds_settings_derive::ConfigEditor;

pub mod actions;
pub mod configuration;
pub mod design;
pub mod desktop;
pub mod i18n;
pub mod notifications;
pub mod terminal;
pub mod theme;
pub mod time;
pub mod typography;
pub mod ui;
