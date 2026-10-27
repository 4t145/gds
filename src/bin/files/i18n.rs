//! Files owns its application messages; GDS supplies Fluent and base messages.

pub use gds::i18n::{Error, Language, format, text};
use gpui_kit::App;

pub fn install(language: Language, cx: &mut App) -> Result<(), Error> {
    let sources = match language {
        Language::English => [
            include_str!("i18n/assets/en-US.ftl"),
            include_str!("i18n/assets/devices-en-US.ftl"),
            include_str!("i18n/assets/operations-en-US.ftl"),
            include_str!("i18n/assets/details-en-US.ftl"),
            include_str!("i18n/assets/font-en-US.ftl"),
            include_str!("i18n/assets/recycle-en-US.ftl"),
        ],
        Language::Chinese => [
            include_str!("i18n/assets/zh-CN.ftl"),
            include_str!("i18n/assets/devices-zh-CN.ftl"),
            include_str!("i18n/assets/operations-zh-CN.ftl"),
            include_str!("i18n/assets/details-zh-CN.ftl"),
            include_str!("i18n/assets/font-zh-CN.ftl"),
            include_str!("i18n/assets/recycle-zh-CN.ftl"),
        ],
    };
    gds::i18n::install_resources(language, &sources, cx)
}
