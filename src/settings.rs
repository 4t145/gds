//! A native settings workspace and the preferences of the current application.

pub mod columns;
mod controls;
mod pages;
mod storage;

pub use pages::Page;
pub use storage::{Error, Loaded, load, load_startup, path, save};

use controls::{
    Choice, ChoiceControl, ChoiceEvent, ChoiceState, FONT_SIZE_RANGE, Field, NumberControl,
    PERCENTAGE_RANGE,
};
use gds_uim::el;
use gpui_kit::{
    component::{
        ActiveTheme, WindowExt,
        slider::{SliderEvent, SliderState, SliderValue},
    },
    *,
};
use serde::{Deserialize, Serialize};

use crate::{
    actions::{Command, SETTINGS_CONTEXT},
    i18n::{Language, text},
    theme, time, typography,
    ui::{Frame, NavItem, SIDEBAR_PADDING, Shortcut},
};

const MIN_LABEL_WIDTH: f32 = 100.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesPreferences {
    pub show_hidden: bool,
    /// Visible columns when no folder-specific override applies.
    pub columns: columns::Layout,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub appearance: theme::Appearance,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
    /// How timestamps are displayed in this application.
    pub time_format: time::Format,
    /// Font family and base text size for this application.
    #[serde(skip_serializing_if = "typography::Preferences::is_default")]
    pub font: typography::Preferences,
    pub files: FilesPreferences,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error(transparent)]
    Theme(#[from] theme::Error),
    #[error(transparent)]
    Language(#[from] crate::i18n::Error),
    #[error(transparent)]
    Typography(#[from] typography::Error),
}

impl Preferences {
    /// Apply shared presentation changes to an initialized application.
    /// Application-specific values are handled by the caller.
    ///
    /// # Errors
    /// Reports invalid theme/material/font values, unavailable palette data or
    /// invalid translations.
    pub fn apply_shared(&self, previous: &Self, cx: &mut App) -> Result<(), ApplyError> {
        let appearance = &self.appearance;
        let old_appearance = &previous.appearance;
        if appearance.source != old_appearance.source || appearance.mode != old_appearance.mode {
            theme::apply(appearance, cx)?;
        } else if appearance != old_appearance {
            theme::update_opacity(appearance.content_opacity, appearance.sidebar_opacity, cx)?;
        }
        if self.font != previous.font {
            typography::install(&self.font, cx)?;
        }
        if self.language != previous.language {
            crate::i18n::install(self.language.unwrap_or_else(Language::detect), cx)?;
        }
        if self.time_format != previous.time_format {
            time::apply(self.time_format, cx);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub enum Event {
    /// An immediate change request; the application applies, persists, then reports the result.
    Apply(Preferences),
}

pub struct View {
    preferences: Preferences,
    sidebar_width: f32,
    page: Page,
    /// One focus target per page, so page cycling keeps keyboard navigation
    /// inside the visible page and blurs controls on the page being left.
    page_focus: [FocusHandle; 5],
    language: ChoiceControl<Option<Language>>,
    time_format: ChoiceControl<time::Format>,
    source: ChoiceControl<theme::Source>,
    mode: ChoiceControl<theme::ModePreference>,
    content_opacity: NumberControl,
    sidebar_opacity: NumberControl,
    /// Installed families, enumerated once per settings view.
    font_families: Vec<String>,
    font_family: ChoiceControl<Option<String>>,
    font_size: NumberControl,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<Event> for View {}

impl View {
    pub fn new(
        preferences: Preferences,
        sidebar_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let language = ChoiceControl::new(
            Choice::<Option<Language>>::catalog(cx),
            &preferences.language,
            true,
            window,
            cx,
        );
        let time_format = ChoiceControl::new(
            Choice::<time::Format>::catalog(cx),
            &preferences.time_format,
            false,
            window,
            cx,
        );
        let source = ChoiceControl::new(
            Choice::<theme::Source>::catalog(cx),
            &preferences.appearance.source,
            false,
            window,
            cx,
        );
        let mode = ChoiceControl::new(
            Choice::<theme::ModePreference>::catalog(cx),
            &preferences.appearance.mode,
            false,
            window,
            cx,
        );
        let content_opacity = NumberControl::new(
            preferences.appearance.content_opacity * 100.0,
            PERCENTAGE_RANGE,
            cx,
        );
        let sidebar_opacity = NumberControl::new(
            preferences.appearance.sidebar_opacity * 100.0,
            PERCENTAGE_RANGE,
            cx,
        );
        let font_families = typography::installed_families(cx);
        let font_family = ChoiceControl::new(
            Choice::<Option<String>>::fonts(preferences.font.family.as_deref(), &font_families, cx),
            &preferences.font.family,
            true,
            window,
            cx,
        );
        let font_size = NumberControl::new(preferences.font.size, FONT_SIZE_RANGE, cx);
        let mut subscriptions = vec![
            cx.subscribe_in(&language.state, window, Self::on_language_confirm),
            cx.subscribe_in(&time_format.state, window, Self::on_time_format_confirm),
            cx.subscribe_in(&source.state, window, Self::on_source_confirm),
            cx.subscribe_in(&mode.state, window, Self::on_mode_confirm),
            cx.subscribe_in(&font_family.state, window, Self::on_font_family_confirm),
        ];
        for (field, state) in [
            (Field::ContentOpacity, content_opacity.state.clone()),
            (Field::SidebarOpacity, sidebar_opacity.state.clone()),
            (Field::FontSize, font_size.state.clone()),
        ] {
            subscriptions.push(cx.subscribe_in(&state, window, Self::on_slider_change(field)));
            // Kit accessibility increments notify the state without a Change event.
            subscriptions.push(cx.observe_in(&state, window, Self::on_slider_notify(field)));
        }
        Self {
            preferences,
            sidebar_width,
            page: Page::General,
            page_focus: std::array::from_fn(|_| cx.focus_handle()),
            language,
            time_format,
            source,
            mode,
            content_opacity,
            sidebar_opacity,
            font_families,
            font_family,
            font_size,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_sidebar_width(&mut self, width: f32, cx: &mut Context<Self>) {
        self.sidebar_width = width;
        cx.notify();
    }

    /// Move to the adjacent settings page, wrapping at both ends of the sidebar.
    ///
    /// Only the visible page changes: file tabs and preference values are
    /// untouched. Focus moves into the new page, which also dismisses a menu or
    /// editor that belonged to the page being left. A modal dialog keeps both
    /// focus and the current page.
    pub fn cycle_page(&mut self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            return;
        }
        let count = Page::ALL.len();
        let current = self.page.index();
        let next = if backwards {
            (current + count - 1) % count
        } else {
            (current + 1) % count
        };
        if Page::ALL[next] == self.page {
            return;
        }
        self.page = Page::ALL[next];
        self.page_focus[next].focus(window, cx);
        cx.notify();
    }

    /// Update the controls after application or rollback, without submitting another request.
    pub fn set_preferences(
        &mut self,
        preferences: Preferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferences = preferences;
        self.sync_controls(window, cx);
        cx.notify();
    }

    fn sync_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.language.sync(
            Choice::<Option<Language>>::catalog(cx),
            &self.preferences.language,
            window,
            cx,
        );
        self.time_format.sync(
            Choice::<time::Format>::catalog(cx),
            &self.preferences.time_format,
            window,
            cx,
        );
        self.source.sync(
            Choice::<theme::Source>::catalog(cx),
            &self.preferences.appearance.source,
            window,
            cx,
        );
        self.mode.sync(
            Choice::<theme::ModePreference>::catalog(cx),
            &self.preferences.appearance.mode,
            window,
            cx,
        );
        self.content_opacity.sync(
            self.preferences.appearance.content_opacity * 100.0,
            window,
            cx,
        );
        self.sidebar_opacity.sync(
            self.preferences.appearance.sidebar_opacity * 100.0,
            window,
            cx,
        );
        self.font_family.sync(
            Choice::<Option<String>>::fonts(
                self.preferences.font.family.as_deref(),
                &self.font_families,
                cx,
            ),
            &self.preferences.font.family,
            window,
            cx,
        );
        self.font_size.sync(self.preferences.font.size, window, cx);
    }

    fn change(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        update: impl FnOnce(&mut Preferences) -> bool,
    ) {
        if !update(&mut self.preferences) {
            return;
        }
        self.sync_controls(window, cx);
        cx.emit(Event::Apply(self.preferences.clone()));
        cx.notify();
    }

    fn on_language_confirm(
        &mut self,
        _: &Entity<ChoiceState<Option<Language>>>,
        event: &ChoiceEvent<Option<Language>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ChoiceEvent::Confirm(Some(value)) = event {
            self.change(window, cx, |preferences| {
                Field::replace(&mut preferences.language, *value)
            });
        }
    }

    fn on_time_format_confirm(
        &mut self,
        _: &Entity<ChoiceState<time::Format>>,
        event: &ChoiceEvent<time::Format>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ChoiceEvent::Confirm(Some(value)) = event {
            self.change(window, cx, |preferences| {
                Field::replace(&mut preferences.time_format, *value)
            });
        }
    }

    fn on_source_confirm(
        &mut self,
        _: &Entity<ChoiceState<theme::Source>>,
        event: &ChoiceEvent<theme::Source>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ChoiceEvent::Confirm(Some(value)) = event {
            self.change(window, cx, |preferences| {
                Field::replace(&mut preferences.appearance.source, *value)
            });
        }
    }

    fn on_mode_confirm(
        &mut self,
        _: &Entity<ChoiceState<theme::ModePreference>>,
        event: &ChoiceEvent<theme::ModePreference>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ChoiceEvent::Confirm(Some(value)) = event {
            self.change(window, cx, |preferences| {
                Field::replace(&mut preferences.appearance.mode, *value)
            });
        }
    }

    fn on_font_family_confirm(
        &mut self,
        _: &Entity<ChoiceState<Option<String>>>,
        event: &ChoiceEvent<Option<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let ChoiceEvent::Confirm(Some(value)) = event {
            let value = value.clone();
            self.change(window, cx, |preferences| {
                Field::replace(&mut preferences.font.family, value)
            });
        }
    }

    fn on_slider_change(
        field: Field,
    ) -> impl Fn(&mut Self, &Entity<SliderState>, &SliderEvent, &mut Window, &mut Context<Self>)
    {
        move |view, _, event, window, cx| {
            if let SliderEvent::Change(value) = event {
                view.number_changed(field, *value, window, cx);
            }
        }
    }

    fn on_slider_notify(
        field: Field,
    ) -> impl Fn(&mut Self, Entity<SliderState>, &mut Window, &mut Context<Self>) {
        move |view, state, window, cx| {
            let value = state.read(cx).value();
            if !view
                .number_control(field)
                .notification_is_programmatic(value)
            {
                view.number_changed(field, value, window, cx);
            }
        }
    }

    fn on_hidden_changed(&mut self, checked: &bool, window: &mut Window, cx: &mut Context<Self>) {
        self.change(window, cx, |preferences| {
            Field::replace(&mut preferences.files.show_hidden, *checked)
        });
    }

    fn on_reset(field: Field) -> impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) {
        move |view, _, window, cx| view.change(window, cx, |preferences| field.reset(preferences))
    }

    fn on_page(
        destination: Page,
    ) -> impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) {
        move |view, _, _, cx| {
            if view.page != destination {
                view.page = destination;
                cx.notify();
            }
        }
    }

    fn number_control(&self, field: Field) -> &NumberControl {
        match field {
            Field::ContentOpacity => &self.content_opacity,
            Field::SidebarOpacity => &self.sidebar_opacity,
            Field::FontSize => &self.font_size,
            _ => unreachable!("only numeric fields own numeric controls"),
        }
    }

    fn number_changed(
        &mut self,
        field: Field,
        value: SliderValue,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let SliderValue::Single(value) = value else {
            return;
        };
        let Some(range) = field.number_range() else {
            return;
        };
        let Some(current) = field.number(&self.preferences) else {
            return;
        };
        let value = range.quantize(value);
        if range.equivalent(current, value) {
            return;
        }
        self.change(window, cx, |preferences| {
            field.set_number(preferences, value)
        });
    }

    fn navigation_shortcut(&self, destination: Page) -> Option<Shortcut> {
        let count = Page::ALL.len();
        let current = self.page.index();
        let next = (current + 1) % count;
        let previous = (current + count - 1) % count;
        let destination = destination.index();
        if destination == next {
            Some(Shortcut::new(
                Command::NextTab.action(),
                Some(SETTINGS_CONTEXT),
            ))
        } else if destination == previous {
            Some(Shortcut::new(
                Command::PreviousTab.action(),
                Some(SETTINGS_CONTEXT),
            ))
        } else {
            None
        }
    }

    fn navigation_item(&self, index: usize, destination: Page, cx: &mut Context<Self>) -> NavItem {
        let id = ("settings-page", index);
        let icon = destination.icon();
        let label = text(destination.label_id(), cx);
        // Holding Alt reveals the real page-cycle binding on the adjacent row.
        let shortcut = self.navigation_shortcut(destination);
        let on_page = cx.listener(Self::on_page(destination));
        let item = el! {
            (id, icon, label).NavItem::new {
                compact: $(self.sidebar_width < MIN_LABEL_WIDTH);
                selected: $(self.page == destination);
                on_click: on_page;
            }
        };
        match shortcut {
            Some(shortcut) => item.shortcut(shortcut),
            None => item,
        }
    }

    fn render_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let items: Vec<_> = Page::ALL
            .into_iter()
            .enumerate()
            .map(|(index, destination)| self.navigation_item(index, destination, cx))
            .collect();
        el! {
            .div { size_full; min_h_0; flex; flex_col; gap_1; p: SIDEBAR_PADDING.px } [..items]
        }
        .into_any_element()
    }

    fn render_page_content(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let page = self.page;
        let content = page.render(self, cx);
        let scope = text("settings-app", cx);
        let title = text(page.label_id(), cx);
        let page_focus = self.page_focus[page.index()].clone();
        el! {
            .div {
                # "settings-page-content";
                track_focus: $(&page_focus);
                size_full; min_w_0; overflow_y_scroll; p_6;
            } [
                .div { flex; flex_col; gap_2; mb_5 } [
                    .div { text_xs; text_color: $(cx.theme().muted_foreground) } [scope],
                    .div { text_xl } [title],
                ],
                content,
            ]
        }
        .into_any_element()
    }
}

impl Render for View {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let navigation = self.render_navigation(cx);
        let content = self.render_page_content(cx);
        let workspace = Frame::new(self.sidebar_width).workspace(navigation, content, cx);
        el! {
            .div { size_full; min_h_0; flex; flex_col; text_color: $(cx.theme().foreground) } [workspace]
        }
    }
}
