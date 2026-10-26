//! Shared presentation and state adapters for native settings controls.

use std::cell::Cell;

use fluent_bundle::FluentArgs;
use gds_uim::el;
use gpui_kit::{
    assets::IconName,
    component::{
        ActiveTheme, IndexPath, Sizable,
        button::{Button, ButtonVariants},
        select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState},
        slider::{Slider, SliderEvent, SliderState, SliderValue},
    },
    *,
};

use crate::{
    i18n::{self, Language, text},
    settings::{Preferences, View},
    theme::{ModePreference, Source},
    time, typography,
};

const CONTROL_WIDTH: f32 = 260.0;
const RESET_WIDTH: f32 = 32.0;
const CONTROL_GAP: f32 = 8.0;
/// Width of the numeric readout slot in `rem`, so it tracks the base text size.
const NUMBER_LABEL_WIDTH_REMS: f32 = 3.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    Language,
    TimeFormat,
    Source,
    Mode,
    ContentOpacity,
    SidebarOpacity,
    ShowHidden,
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

#[derive(Clone, Copy, Debug)]
pub(super) struct NumberRange {
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

impl NumberRange {
    pub fn quantize(self, value: f32) -> f32 {
        if !value.is_finite() {
            return self.max;
        }
        let value = value.clamp(self.min, self.max);
        (((value - self.min) / self.step).round() * self.step + self.min).clamp(self.min, self.max)
    }

    pub fn equivalent(self, left: f32, right: f32) -> bool {
        left.is_finite() && right.is_finite() && self.quantize(left) == self.quantize(right)
    }
}

pub(super) const PERCENTAGE_RANGE: NumberRange = NumberRange {
    min: 0.0,
    max: 100.0,
    step: 1.0,
};

pub(super) const FONT_SIZE_RANGE: NumberRange = NumberRange {
    min: typography::MIN_SIZE,
    max: typography::MAX_SIZE,
    step: 1.0,
};

pub(super) struct NumberControl {
    pub state: Entity<SliderState>,
    focus: FocusHandle,
    pub range: NumberRange,
    programmatic_value: Cell<Option<f32>>,
}

impl NumberControl {
    pub fn new(value: f32, range: NumberRange, cx: &mut Context<View>) -> Self {
        let state = cx.new(|_| {
            SliderState::new()
                .min(range.min)
                .max(range.max)
                .step(range.step)
                .default_value(range.quantize(value))
        });
        Self {
            state,
            focus: cx.focus_handle(),
            range,
            programmatic_value: Cell::new(None),
        }
    }

    /// Keeping equal state untouched also preserves an active drag.
    pub fn sync(&self, value: f32, window: &mut Window, cx: &mut Context<View>) {
        let value = self.range.quantize(value);
        if self.state.read(cx).value() != SliderValue::Single(value) {
            self.programmatic_value.set(Some(value));
            self.state
                .update(cx, |state, cx| state.set_value(value, window, cx));
        }
    }

    pub fn notification_is_programmatic(&self, value: SliderValue) -> bool {
        self.programmatic_value
            .take()
            .is_some_and(|expected| value == SliderValue::Single(expected))
    }

    fn pointer_focus_handler(&self) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + use<> {
        let focus = self.focus.clone();
        move |_, window, cx| focus.focus(window, cx)
    }

    fn key_handler(&self) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + use<> {
        let state = self.state.clone();
        let range = self.range;
        move |event, window, cx| {
            let modifiers = event.keystroke.modifiers;
            if modifiers.control || modifiers.alt || modifiers.platform {
                return;
            }
            let current = state.read(cx).value().end();
            let next = match event.keystroke.key.as_str() {
                "left" | "down" => current - range.step,
                "right" | "up" => current + range.step,
                "home" => range.min,
                "end" => range.max,
                _ => return,
            };
            let next = range.quantize(next);
            cx.stop_propagation();
            window.prevent_default();
            if next != current {
                state.update(cx, |state, cx| {
                    state.set_value(next, window, cx);
                    cx.emit(SliderEvent::Change(SliderValue::Single(next)));
                });
            }
        }
    }

    pub fn render(
        &self,
        field: Field,
        value_label: SharedString,
        cx: &App,
    ) -> impl IntoElement + use<> {
        let on_pointer_down = self.pointer_focus_handler();
        let on_key_down = self.key_handler();
        el! {
            .div {
                # $("settings-number", field as usize);
                track_focus: $(&self.focus);
                tab_index: 0; tab_stop: true;
                flex; items_center; gap: CONTROL_GAP.px; w_full;
                on_mouse_down: MouseButton::Left, on_pointer_down;
                on_key_down: on_key_down;
            } [
                $(&self.state).Slider::new {
                    accessibility_label: $(text(field.label_id(), cx));
                    flex_1;
                },
                // The value and its unit share one unbreakable line.
                .div {
                    w: $(rems(NUMBER_LABEL_WIDTH_REMS));
                    flex_shrink_0; whitespace_nowrap;
                    text_right; text_sm;
                } [value_label],
            ]
        }
    }
}

#[derive(IntoElement)]
pub(super) struct SettingRow {
    title: SharedString,
    control: AnyElement,
    reset: Option<Button>,
}

impl SettingRow {
    pub fn new(title: SharedString, control: impl IntoElement) -> Self {
        Self {
            title,
            control: control.into_any_element(),
            reset: None,
        }
    }

    pub fn editable(
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
        Self {
            title: text(field.label_id(), cx),
            control: control.into_any_element(),
            reset,
        }
    }
}

impl RenderOnce for SettingRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            title,
            control,
            reset,
        } = self;
        el! {
            .div {
                flex; items_center; justify_between; gap_6; py_4;
                border_b_1; border_color: $(cx.theme().border);
            } [
                .div { flex_1; min_w_0; text_sm } [title],
                .div { flex; items_center; gap: CONTROL_GAP.px } [
                    .div { w: RESET_WIDTH.px; h: RESET_WIDTH.px; flex; items_center; justify_center } [..reset],
                    .div { w: CONTROL_WIDTH.px; flex; items_center; justify_end } [control],
                ],
            ]
        }
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
