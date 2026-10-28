use super::{Field, NumberRange, PERCENTAGE_RANGE};
use crate::settings::{
    FilesPreferences, Preferences,
    columns::{Column, Layout},
};
use gds::ui::settings::{FONT_SIZE_RANGE, form::Model};
use gds::{
    i18n::Language,
    theme::{Appearance, ModePreference, Source},
    time::Format,
    typography,
};

fn modified_preferences() -> Preferences {
    Preferences {
        appearance: Appearance {
            source: Source::Builtin,
            mode: ModePreference::Light,
            content_opacity: 0.7,
            sidebar_opacity: 0.8,
        },
        language: Some(Language::English),
        time_format: Format::Absolute,
        font: typography::Preferences {
            family: Some("Noto Sans".to_owned()),
            size: 18.0,
        },
        files: FilesPreferences {
            sidebar: Default::default(),
            volume_mount_points: Default::default(),
            show_hidden: true,
            columns: Layout::new([
                Column::Name,
                Column::Owner,
                Column::DetailedType,
                Column::Modified,
            ]),
        },
    }
}

fn reset_generated<T: Model>(model: &mut T, id: &str) {
    let field = T::fields()
        .into_iter()
        .find(|field| field.id == id)
        .expect("generated field");
    (field.set)(model, (field.get)(&T::default())).expect("default field value");
}

#[test]
fn resetting_a_theme_choice_does_not_reset_the_other_theme_fields() {
    let before = modified_preferences();
    let mut source = before.clone();
    assert!(Field::Source.reset(&mut source));
    assert_eq!(
        source.appearance.source,
        Preferences::default().appearance.source
    );
    assert_eq!(source.appearance.mode, before.appearance.mode);
    assert_eq!(
        source.appearance.content_opacity,
        before.appearance.content_opacity
    );
    assert_eq!(
        source.appearance.sidebar_opacity,
        before.appearance.sidebar_opacity
    );
    assert_eq!(source.language, before.language);
    assert_eq!(source.files, before.files);

    let mut mode = before.clone();
    assert!(Field::Mode.reset(&mut mode));
    assert_eq!(mode.appearance.mode, Preferences::default().appearance.mode);
    assert_eq!(mode.appearance.source, before.appearance.source);
    assert_eq!(
        mode.appearance.content_opacity,
        before.appearance.content_opacity
    );
    assert_eq!(
        mode.appearance.sidebar_opacity,
        before.appearance.sidebar_opacity
    );
}

#[test]
fn resetting_one_opacity_keeps_the_other_material_unchanged() {
    let before = modified_preferences();
    let mut content = before.clone();
    assert!(Field::ContentOpacity.reset(&mut content));
    assert_eq!(
        content.appearance.content_opacity,
        Preferences::default().appearance.content_opacity
    );
    assert_eq!(content.appearance.sidebar_opacity, 0.8);
    assert_eq!(content.appearance.source, Source::Builtin);
    assert_eq!(content.appearance.mode, ModePreference::Light);
    assert_eq!(content.language, before.language);
    assert_eq!(content.files, before.files);

    let mut sidebar = before;
    assert!(Field::SidebarOpacity.reset(&mut sidebar));
    assert_eq!(
        sidebar.appearance.sidebar_opacity,
        Preferences::default().appearance.sidebar_opacity
    );
    assert_eq!(sidebar.appearance.content_opacity, 0.7);
    assert_eq!(sidebar.appearance.source, Source::Builtin);
    assert_eq!(sidebar.appearance.mode, ModePreference::Light);
}

#[test]
fn language_and_file_preference_reset_are_independent() {
    let before = modified_preferences();
    let mut language = before.clone();
    reset_generated(&mut language, "language");
    assert_eq!(language.language, Preferences::default().language);
    assert_eq!(language.appearance, before.appearance);
    assert_eq!(language.files, before.files);

    let mut files = before.clone();
    assert!(Field::ShowHidden.reset(&mut files));
    assert!(!files.files.show_hidden);
    assert_eq!(files.files.columns, before.files.columns);
    assert_eq!(files.language, before.language);
    assert_eq!(files.appearance, before.appearance);
}

#[test]
fn font_family_and_size_reset_independently_of_every_other_preference() {
    let before = modified_preferences();

    let mut family = before.clone();
    reset_generated(&mut family.font, "family");
    assert_eq!(family.font.family, Preferences::default().font.family);
    assert_eq!(family.font.size, before.font.size);
    assert_eq!(family.appearance, before.appearance);
    assert_eq!(family.language, before.language);
    assert_eq!(family.time_format, before.time_format);
    assert_eq!(family.files, before.files);

    let mut size = before.clone();
    reset_generated(&mut size.font, "size");
    assert_eq!(size.font.size, Preferences::default().font.size);
    assert_eq!(size.font.family, before.font.family);
    assert_eq!(size.files, before.files);
}

#[test]
fn time_format_reset_is_independent_of_files_language_and_columns() {
    let before = modified_preferences();
    assert_ne!(before.time_format, Preferences::default().time_format);

    let mut time = before.clone();
    reset_generated(&mut time, "time_format");
    assert_eq!(time.time_format, Preferences::default().time_format);
    assert_eq!(time.files, before.files);
    assert_eq!(time.language, before.language);
    assert_eq!(time.appearance, before.appearance);

    let mut files = before.clone();
    assert!(Field::ShowHidden.reset(&mut files));
    assert_eq!(files.time_format, before.time_format);
    assert_eq!(files.files.columns, before.files.columns);

    let mut language = before.clone();
    reset_generated(&mut language, "language");
    assert_eq!(language.time_format, before.time_format);
}

#[test]
fn numeric_ranges_quantize_relative_to_the_minimum_and_clamp() {
    let range = NumberRange {
        min: 5.0,
        max: 15.0,
        step: 2.0,
    };
    assert_eq!(range.quantize(8.0), 9.0);
    assert_eq!(range.quantize(3.0), 5.0);
    assert_eq!(range.quantize(20.0), 15.0);
    assert_eq!(PERCENTAGE_RANGE.quantize(69.4), 69.0);
    assert_eq!(PERCENTAGE_RANGE.quantize(69.8), 70.0);
    assert_eq!(FONT_SIZE_RANGE.quantize(9.0), typography::MIN_SIZE);
    assert_eq!(FONT_SIZE_RANGE.quantize(30.0), typography::MAX_SIZE);
    assert_eq!(FONT_SIZE_RANGE.quantize(16.4), typography::DEFAULT_SIZE);
}

#[test]
fn legacy_float_rounding_is_equivalent_without_rewriting_the_preference() {
    assert!(PERCENTAGE_RANGE.equivalent(0.6999999 * 100.0, 70.0));
    assert!(!PERCENTAGE_RANGE.equivalent(69.0, 70.0));
    let mut preferences = Preferences::default();
    let rounded = preferences.appearance.content_opacity - f32::EPSILON;
    preferences.appearance.content_opacity = rounded;
    assert!(!Field::ContentOpacity.is_modified(&preferences));
    assert!(!Field::ContentOpacity.reset(&mut preferences));
    assert_eq!(preferences.appearance.content_opacity, rounded);
}
