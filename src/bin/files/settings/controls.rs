//! Shared presentation and state adapters for native settings controls.

use fluent_bundle::FluentArgs;
use gds_uim::el;
use gpui_kit::{
    assets::IconName,
    component::{
        ActiveTheme, IndexPath, Sizable,
        button::{Button, ButtonVariants},
        select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState},
    },
    *,
};

use crate::settings::{Preferences, View};
use gds::{
    i18n::{self, Language, text},
    theme::{ModePreference, Source},
    time,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    Language,
    TimeFormat,
    Source,
    Mode,
    ContentOpacity,
    SidebarOpacity,
    ShowHidden,
    SidebarVolumes,
    FontFamily,
    FontSize,
}

impl Field {
    pub fn label_id(self) -> &'static str {
        match self {
            Self::Language => "language",
            Self::TimeFormat => "time-format-label",
            Self::Source => "theme-source",
            Self::Mode => "theme-mode",
            Self::ContentOpacity => "theme-opacity",
            Self::SidebarOpacity => "sidebar-opacity",
            Self::ShowHidden => "show-hidden",
            Self::SidebarVolumes => "sidebar-show-volumes",
            Self::FontFamily => "font-family-label",
            Self::FontSize => "font-size-label",
        }
    }

    pub fn is_modified(self, preferences: &Preferences) -> bool {
        let defaults = Preferences::default();
        match self {
            Self::Language => preferences.language != defaults.language,
            Self::TimeFormat => preferences.time_format != defaults.time_format,
            Self::Source => preferences.appearance.source != defaults.appearance.source,
            Self::Mode => preferences.appearance.mode != defaults.appearance.mode,
            Self::ContentOpacity => !PERCENTAGE_RANGE.equivalent(
                preferences.appearance.content_opacity * 100.0,
                defaults.appearance.content_opacity * 100.0,
            ),
            Self::SidebarOpacity => !PERCENTAGE_RANGE.equivalent(
                preferences.appearance.sidebar_opacity * 100.0,
                defaults.appearance.sidebar_opacity * 100.0,
            ),
            Self::ShowHidden => preferences.files.show_hidden != defaults.files.show_hidden,
            Self::SidebarVolumes => {
                preferences.files.sidebar.show_volumes != defaults.files.sidebar.show_volumes
            }
            Self::FontFamily => preferences.font.family != defaults.font.family,
            Self::FontSize => {
                !FONT_SIZE_RANGE.equivalent(preferences.font.size, defaults.font.size)
            }
        }
    }

    pub fn reset(self, preferences: &mut Preferences) -> bool {
        if !self.is_modified(preferences) {
            return false;
        }
        let defaults = Preferences::default();
        match self {
            Self::Language => preferences.language = defaults.language,
            Self::TimeFormat => preferences.time_format = defaults.time_format,
            Self::Source => preferences.appearance.source = defaults.appearance.source,
            Self::Mode => preferences.appearance.mode = defaults.appearance.mode,
            Self::ContentOpacity => {
                preferences.appearance.content_opacity = defaults.appearance.content_opacity
            }
            Self::SidebarOpacity => {
                preferences.appearance.sidebar_opacity = defaults.appearance.sidebar_opacity
            }
            Self::ShowHidden => preferences.files.show_hidden = defaults.files.show_hidden,
            Self::SidebarVolumes => {
                preferences.files.sidebar.show_volumes = defaults.files.sidebar.show_volumes
            }
            Self::FontFamily => preferences.font.family = defaults.font.family,
            Self::FontSize => preferences.font.size = defaults.font.size,
        }
        true
    }

    pub fn replace<T: PartialEq>(target: &mut T, value: T) -> bool {
        if *target == value {
            return false;
        }
        *target = value;
        true
    }

    pub fn number_range(self) -> Option<NumberRange> {
        match self {
            Self::ContentOpacity | Self::SidebarOpacity => Some(PERCENTAGE_RANGE),
            Self::FontSize => Some(FONT_SIZE_RANGE),
            _ => None,
        }
    }

    pub fn number(self, preferences: &Preferences) -> Option<f32> {
        match self {
            Self::ContentOpacity => Some(preferences.appearance.content_opacity * 100.0),
            Self::SidebarOpacity => Some(preferences.appearance.sidebar_opacity * 100.0),
            Self::FontSize => Some(preferences.font.size),
            _ => None,
        }
    }

    pub fn set_number(self, preferences: &mut Preferences, value: f32) -> bool {
        match self {
            Self::ContentOpacity => {
                Self::replace(&mut preferences.appearance.content_opacity, value / 100.0)
            }
            Self::SidebarOpacity => {
                Self::replace(&mut preferences.appearance.sidebar_opacity, value / 100.0)
            }
            Self::FontSize => Self::replace(&mut preferences.font.size, value),
            _ => unreachable!("only numeric fields own numeric controls"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Choice<T> {
    value: T,
    title: SharedString,
    search_terms: &'static str,
}

impl<T> Choice<T> {
    fn new(value: T, title: SharedString, search_terms: &'static str) -> Self {
        Self {
            value,
            title,
            search_terms,
        }
    }
}

impl<T: Clone + PartialEq> SelectItem for Choice<T> {
    type Value = T;
    fn title(&self) -> SharedString {
        self.title.clone()
    }
    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.title.to_lowercase().contains(&query)
            || self.search_terms.to_lowercase().contains(&query)
    }
}

struct LanguageEntry {
    value: Option<Language>,
    label_id: &'static str,
    search_terms: &'static str,
}

const SUPPORTED_LANGUAGES: &[LanguageEntry] = &[
    LanguageEntry {
        value: None,
        label_id: "language-system",
        search_terms: "system automatic 系统",
    },
    LanguageEntry {
        value: Some(Language::Chinese),
        label_id: "language-chinese",
        search_terms: "zh zh-CN Chinese 中文 简体",
    },
    LanguageEntry {
        value: Some(Language::English),
        label_id: "language-english",
        search_terms: "en en-US English 英语 英文",
    },
];

impl Choice<Option<Language>> {
    pub fn catalog(cx: &App) -> Vec<Self> {
        SUPPORTED_LANGUAGES
            .iter()
            .map(|entry| Self::new(entry.value, text(entry.label_id, cx), entry.search_terms))
            .collect()
    }
}

impl Choice<time::Format> {
    pub fn catalog(cx: &App) -> Vec<Self> {
        vec![
            Self::new(
                time::Format::Relative,
                text("time-format-relative", cx),
                "relative ago today 相对 之前 刚刚",
            ),
            Self::new(
                time::Format::Absolute,
                text("time-format-absolute", cx),
                "absolute date time 完整 日期 时间",
            ),
        ]
    }
}

impl Choice<Source> {
    pub fn catalog(cx: &App) -> Vec<Self> {
        vec![
            Self::new(Source::Dms, text("theme-system", cx), "system 系统"),
            Self::new(Source::Builtin, text("theme-builtin", cx), "builtin 内置"),
        ]
    }
}

impl Choice<ModePreference> {
    pub fn catalog(cx: &App) -> Vec<Self> {
        vec![
            Self::new(
                ModePreference::System,
                text("theme-mode-system", cx),
                "system automatic 系统",
            ),
            Self::new(ModePreference::Light, text("theme-light", cx), "light 浅色"),
            Self::new(ModePreference::Dark, text("theme-dark", cx), "dark 深色"),
        ]
    }
}

impl Choice<Option<String>> {
    /// The system default, then every installed family, then a configured
    /// family that is no longer installed so its stored value stays visible.
    ///
    /// The installed list is passed in because enumerating platform fonts is
    /// more expensive than rebuilding the labels after a language change.
    pub fn fonts(current: Option<&str>, installed: &[String], cx: &App) -> Vec<Self> {
        let mut choices = vec![Self::new(
            None,
            text("font-family-system", cx),
            "system default 系统 默认",
        )];
        choices.extend(
            installed
                .iter()
                .map(|family| Self::new(Some(family.clone()), family.clone().into(), "")),
        );
        if let Some(family) =
            current.filter(|family| !installed.iter().any(|name| name.as_str() == *family))
        {
            let mut arguments = FluentArgs::new();
            arguments.set("family", family.to_owned());
            choices.push(Self::new(
                Some(family.to_owned()),
                i18n::format("font-family-missing", Some(&arguments), cx),
                "",
            ));
        }
        choices
    }
}

pub(super) type ChoiceEvent<T> = SelectEvent<SearchableVec<Choice<T>>>;
pub(super) type ChoiceState<T> = SelectState<SearchableVec<Choice<T>>>;

pub(super) struct ChoiceControl<T: Clone + PartialEq + 'static> {
    pub state: Entity<ChoiceState<T>>,
    choices: Vec<Choice<T>>,
}

impl<T: Clone + PartialEq + 'static> ChoiceControl<T> {
    pub fn new(
        choices: Vec<Choice<T>>,
        value: &T,
        searchable: bool,
        window: &mut Window,
        cx: &mut Context<View>,
    ) -> Self {
        let selected = choices
            .iter()
            .position(|choice| &choice.value == value)
            .map(|row| IndexPath::default().row(row));
        let items = SearchableVec::new(choices.clone());
        let state =
            cx.new(|cx| SelectState::new(items, selected, window, cx).searchable(searchable));
        Self { state, choices }
    }

    /// Kit's programmatic setters notify but do not emit `SelectEvent::Confirm`.
    pub fn sync(
        &mut self,
        choices: Vec<Choice<T>>,
        value: &T,
        window: &mut Window,
        cx: &mut Context<View>,
    ) {
        if self.choices != choices {
            self.state.update(cx, |state, cx| {
                state.set_items(SearchableVec::new(choices.clone()), window, cx)
            });
            self.choices = choices;
        }
        let expected = self
            .choices
            .iter()
            .find(|choice| &choice.value == value)
            .map(|choice| choice.value.clone());
        if self.state.read(cx).selected_value().cloned() == expected {
            return;
        }
        self.state.update(cx, |state, cx| match &expected {
            Some(value) => state.set_selected_value(value, window, cx),
            None => state.set_selected_index(None, window, cx),
        });
    }

    pub fn render(
        &self,
        field: Field,
        placeholder: SharedString,
        search_placeholder: SharedString,
        cx: &App,
    ) -> Select<SearchableVec<Choice<T>>> {
        el! {
            $(&self.state).Select::new {
                id: $("settings-select", field as usize);
                small; w_full;
                menu_width: CONTROL_WIDTH.px;
                accessibility_label: $(text(field.label_id(), cx));
                placeholder: placeholder;
                search_placeholder: search_placeholder;
            }
        }
    }
}

pub(super) use gds::ui::settings::{CONTROL_WIDTH, FONT_SIZE_RANGE, NumberRange, PERCENTAGE_RANGE};

pub(super) struct NumberControl(gds::ui::settings::NumberControl);
impl std::ops::Deref for NumberControl {
    type Target = gds::ui::settings::NumberControl;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl NumberControl {
    pub fn new(value: f32, range: NumberRange, cx: &mut Context<View>) -> Self {
        Self(gds::ui::settings::NumberControl::new(value, range, cx))
    }
    pub fn render(
        &self,
        field: Field,
        value_label: SharedString,
        cx: &App,
    ) -> impl IntoElement + use<> {
        self.0.element(
            ("settings-number", field as usize).into(),
            text(field.label_id(), cx),
            value_label,
            false,
        )
    }
}

#[derive(IntoElement)]
pub(super) struct SettingRow(gds::ui::settings::SettingRow);
impl RenderOnce for SettingRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.0.render(window, cx)
    }
}

impl SettingRow {
    pub fn new(title: SharedString, control: impl IntoElement) -> Self {
        Self(gds::ui::settings::SettingRow::new(title, control))
    }

    pub(super) fn editable(
        field: Field,
        view: &View,
        control: impl IntoElement,
        cx: &mut Context<View>,
    ) -> Self {
        let reset = if field.is_modified(&view.preferences) {
            let label = text("settings-reset-default", cx);
            let on_reset = cx.listener(View::on_reset(field));
            Some(el! {
                $("settings-reset", field as usize).Button::new {
                    ghost; small; icon: IconName::RotateCcw;
                    accessibility_label: ~label;
                    tooltip: label;
                    on_click: on_reset;
                }
            })
        } else {
            None
        };
        Self(
            gds::ui::settings::SettingRow::new(text(field.label_id(), cx), control)
                .reset(reset.map(IntoElement::into_any_element)),
        )
    }
}

#[derive(IntoElement)]
pub(super) struct SettingNote {
    message: SharedString,
}

impl SettingNote {
    pub fn new(message: SharedString) -> Self {
        Self { message }
    }
}

impl RenderOnce for SettingNote {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        el! { .div { text_sm; text_color: $(cx.theme().muted_foreground) } [$self.message] }
    }
}

#[cfg(test)]
#[path = "controls/tests.rs"]
mod tests;
