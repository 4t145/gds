//! Application messages use Fluent; component translations share the same locale.

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use gpui_kit::{App, Global, SharedString};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use unic_langid::LanguageIdentifier;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    pub fn detect() -> Self {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(|name| std::env::var(name).ok())
            .find(|value| !value.is_empty());
        if locale.is_some_and(|value| value.starts_with("zh")) {
            Self::Chinese
        } else {
            Self::English
        }
    }

    pub fn locale(self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::Chinese => "zh-CN",
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid bundled translation: {0}")]
    Resource(String),
    #[error("could not register translations: {0}")]
    Bundle(String),
}

struct Messages {
    bundle: FluentBundle<FluentResource>,
}

impl Global for Messages {}

/// Install application and component translations.
///
/// Application messages, shared timestamp messages, file operations, the
/// details panel, the typography settings and the trash view are bundled
/// separately so each area owns its keys; all of them land in one locale
/// bundle, so message lookup and formatting stay unchanged.
///
/// # Errors
/// Returns an error when a bundled Fluent resource cannot be parsed or registered.
pub fn install(language: Language, cx: &mut App) -> Result<(), Error> {
    let resources: [(&str, &str); 6] = match language {
        Language::English => [
            ("en-US.ftl", include_str!("i18n/assets/en-US.ftl")),
            ("time-en-US.ftl", include_str!("i18n/assets/time-en-US.ftl")),
            (
                "operations-en-US.ftl",
                include_str!("i18n/assets/operations-en-US.ftl"),
            ),
            (
                "details-en-US.ftl",
                include_str!("i18n/assets/details-en-US.ftl"),
            ),
            ("font-en-US.ftl", include_str!("i18n/assets/font-en-US.ftl")),
            (
                "recycle-en-US.ftl",
                include_str!("i18n/assets/recycle-en-US.ftl"),
            ),
        ],
        Language::Chinese => [
            ("zh-CN.ftl", include_str!("i18n/assets/zh-CN.ftl")),
            ("time-zh-CN.ftl", include_str!("i18n/assets/time-zh-CN.ftl")),
            (
                "operations-zh-CN.ftl",
                include_str!("i18n/assets/operations-zh-CN.ftl"),
            ),
            (
                "details-zh-CN.ftl",
                include_str!("i18n/assets/details-zh-CN.ftl"),
            ),
            ("font-zh-CN.ftl", include_str!("i18n/assets/font-zh-CN.ftl")),
            (
                "recycle-zh-CN.ftl",
                include_str!("i18n/assets/recycle-zh-CN.ftl"),
            ),
        ],
    };
    let locale: LanguageIdentifier = language
        .locale()
        .parse()
        .expect("bundled locale identifiers are valid BCP 47");
    let mut bundle = FluentBundle::new(vec![locale]);
    bundle.set_use_isolating(false);
    for (name, source) in resources {
        let resource = FluentResource::try_new(source.to_owned())
            .map_err(|(_, errors)| Error::Resource(format!("{name}: {errors:?}")))?;
        bundle
            .add_resource(resource)
            .map_err(|errors| Error::Bundle(format!("{name}: {errors:?}")))?;
    }
    gpui_kit::component::set_locale(language.locale());
    cx.set_global(Messages { bundle });
    // Text looked up while rendering picks up the new bundle on the next frame;
    // controls that cache their own copy are refreshed by their owning view.
    cx.refresh_windows();
    Ok(())
}

pub fn text(id: &str, cx: &App) -> SharedString {
    format(id, None, cx)
}

pub fn format(id: &str, arguments: Option<&FluentArgs<'_>>, cx: &App) -> SharedString {
    let messages = cx.global::<Messages>();
    let Some(pattern) = messages
        .bundle
        .get_message(id)
        .and_then(|message| message.value())
    else {
        eprintln!("Missing Fluent message: {id}");
        return id.to_owned().into();
    };
    let mut errors = Vec::new();
    let text = messages
        .bundle
        .format_pattern(pattern, arguments, &mut errors)
        .into_owned();
    if !errors.is_empty() {
        eprintln!("Could not format Fluent message {id}: {errors:?}");
    }
    text.into()
}

pub fn count(id: &str, count: usize, cx: &App) -> SharedString {
    let mut arguments = FluentArgs::new();
    arguments.set("count", count as i64);
    format(id, Some(&arguments), cx)
}
