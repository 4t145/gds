use std::path::Path;

use gpui_kit::{Hsla, WindowAppearance, rgb};

use super::{Appearance, Error, Mode, ModePreference, Source, dms, qt, validate_opacity};

fn color(value: u32) -> Hsla {
    rgb(value).into()
}

#[test]
fn appearance_defaults_follow_system_without_changing_explicit_legacy_choices() {
    let appearance: Appearance = serde_json::from_str("{}").expect("valid settings JSON");
    assert_eq!(appearance, Appearance::default());
    assert_eq!(appearance.source, Source::Dms);
    assert_eq!(appearance.mode, ModePreference::System);
    for (stored, expected) in [
        ("light", ModePreference::Light),
        ("dark", ModePreference::Dark),
    ] {
        let source = format!("source = 'builtin'\nmode = '{stored}'\n");
        let legacy: Appearance = toml::from_str(&source).expect("legacy appearance configuration");
        assert_eq!(legacy.source, Source::Builtin);
        assert_eq!(legacy.mode, expected);
        let encoded = serde_json::to_value(legacy).expect("serializable appearance");
        assert_eq!(encoded["mode"], stored);
    }
}

#[test]
fn builtin_fallback_keeps_the_mode_intent_and_uses_safe_material_defaults() {
    for mode in [
        ModePreference::System,
        ModePreference::Light,
        ModePreference::Dark,
    ] {
        let requested = Appearance {
            source: Source::Dms,
            mode,
            content_opacity: f32::NAN,
            sidebar_opacity: -1.0,
        };
        let fallback = requested.builtin_fallback();
        assert_eq!(fallback.source, Source::Builtin);
        assert_eq!(fallback.mode, mode);
        assert_eq!(
            fallback.content_opacity,
            Appearance::default().content_opacity
        );
        assert_eq!(
            fallback.sidebar_opacity,
            Appearance::default().sidebar_opacity
        );
        assert_eq!(requested.source, Source::Dms);
        assert!(requested.content_opacity.is_nan());
    }
}

#[test]
fn platform_appearance_is_only_used_for_system_mode() {
    assert_eq!(
        ModePreference::System.resolve(WindowAppearance::VibrantLight),
        Mode::Light
    );
    assert_eq!(
        ModePreference::System.resolve(WindowAppearance::VibrantDark),
        Mode::Dark
    );
    assert_eq!(
        ModePreference::Light.resolve(WindowAppearance::Dark),
        Mode::Light
    );
    assert_eq!(
        ModePreference::Dark.resolve(WindowAppearance::Light),
        Mode::Dark
    );
}

#[test]
fn invalid_opacity_is_rejected_instead_of_silently_clamped() {
    for value in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(matches!(
            validate_opacity("content_opacity", value),
            Err(Error::InvalidOpacity { .. })
        ));
    }
    assert!(validate_opacity("content_opacity", 0.0).is_ok());
    assert!(validate_opacity("content_opacity", 1.0).is_ok());
}

fn dms_export(active_mode: &str) -> String {
    serde_json::json!({
        "mode": active_mode,
        "colors": {
            "dark": {"surface":"#101010", "on_surface":"#eeeeee", "primary":"#888888", "on_primary":"#111111"},
            "light": {"surface":"#abcdef", "on_surface":"#123456", "primary":"#654321", "on_primary":"#fedcba"}
        },
        "dank16": {"not-ui-data": true}
    }).to_string()
}

#[test]
fn dms_system_mode_selects_the_exported_active_scheme() {
    for (active, expected, surface) in [
        ("light", Mode::Light, 0xabcdef),
        ("dark", Mode::Dark, 0x101010),
    ] {
        let parsed = dms::parse(
            &dms_export(active),
            Path::new("dms-colors.json"),
            ModePreference::System,
        )
        .expect("active DMS scheme exists");
        assert_eq!(parsed.mode, expected);
        assert_eq!(parsed.palette.content, color(surface));
    }
}

#[test]
fn dms_active_mode_is_validated_only_when_following_system() {
    for metadata in [
        None,
        Some(serde_json::json!("unknown")),
        Some(serde_json::json!(42)),
    ] {
        let mut export: serde_json::Value =
            serde_json::from_str(&dms_export("light")).expect("valid fixture");
        let object = export.as_object_mut().expect("DMS export is an object");
        if let Some(value) = metadata {
            object.insert("mode".into(), value);
        } else {
            object.remove("mode");
        }
        let source = export.to_string();
        assert!(matches!(
            dms::parse(&source, Path::new("dms-colors.json"), ModePreference::System),
            Err(Error::Invalid { field, .. }) if field == "mode"
        ));
        let manual = dms::parse(&source, Path::new("dms-colors.json"), ModePreference::Dark)
            .expect("unrelated active-mode metadata must not block an explicit choice");
        assert_eq!(manual.mode, Mode::Dark);
    }
}

#[test]
fn dms_selects_dark_when_the_export_is_active_light() {
    let parsed = dms::parse(
        &dms_export("light"),
        Path::new("dms-colors.json"),
        ModePreference::Dark,
    )
    .expect("requested dark scheme exists");
    assert_eq!(parsed.mode, Mode::Dark);
    assert_eq!(parsed.palette.content, color(0x101010));
    assert_eq!(parsed.palette.foreground, color(0xeeeeee));
    assert_eq!(parsed.palette.selection, color(0x888888));
    assert_eq!(parsed.palette.selection_foreground, color(0x111111));
}

#[test]
fn dms_selects_light_when_the_export_is_active_dark() {
    let parsed = dms::parse(
        &dms_export("dark"),
        Path::new("dms-colors.json"),
        ModePreference::Light,
    )
    .expect("requested light scheme exists");
    assert_eq!(parsed.mode, Mode::Light);
    assert_eq!(parsed.palette.content, color(0xabcdef));
    assert_eq!(parsed.palette.foreground, color(0x123456));
    assert_eq!(parsed.palette.selection, color(0x654321));
    assert_eq!(parsed.palette.selection_foreground, color(0xfedcba));
}

#[test]
fn dms_missing_requested_scheme_does_not_use_the_available_other_scheme() {
    for (requested, source, missing_field) in [
        (
            ModePreference::Dark,
            r##"{"mode":"light","colors":{"light":{"surface":"#ffffff","on_surface":"#000000","primary":"#123456","on_primary":"#ffffff"}}}"##,
            "colors.dark",
        ),
        (
            ModePreference::Light,
            r##"{"mode":"dark","colors":{"dark":{"surface":"#000000","on_surface":"#ffffff","primary":"#abcdef","on_primary":"#000000"}}}"##,
            "colors.light",
        ),
        (
            ModePreference::System,
            r##"{"mode":"dark","colors":{"light":{"surface":"#ffffff","on_surface":"#000000","primary":"#123456","on_primary":"#ffffff"}}}"##,
            "colors.dark",
        ),
    ] {
        assert!(matches!(
            dms::parse(source, Path::new("dms-colors.json"), requested),
            Err(Error::Invalid { field, .. }) if field == missing_field
        ));
    }
}

#[test]
fn dms_rejects_missing_roles_and_incomplete_color_pairs() {
    for source in [
        r##"{"mode":"dark","colors":{"light":{}}}"##,
        r##"{"mode":"dark","colors":{"dark":{"surface":"#111111"}}}"##,
        r##"{"mode":"dark","colors":{"dark":{"surface":"#111111","on_surface":"#eeeeee","primary":"#abcdef","on_primary":"#123456","primary_container":"#222222"}}}"##,
        r##"{"mode":"dark","colors":{"dark":{"surface":"not-a-color","on_surface":"#eeeeee","primary":"#abcdef","on_primary":"#123456"}}}"##,
    ] {
        assert!(dms::parse(source, Path::new("dms-colors.json"), ModePreference::Dark).is_err());
    }
}

#[test]
fn qt_configuration_controls_the_source_path() {
    let config = Path::new("/config/qt6ct/qt6ct.conf");
    let home = Path::new("/home/test");
    let relative =
        "[Appearance]\ncustom_palette=true\ncolor_scheme_path=\"colors/My theme.conf\"\n";
    assert_eq!(
        qt::palette_path(relative, config, home).expect("relative palette path"),
        Path::new("/config/qt6ct/colors/My theme.conf")
    );
    let tilde = "[Appearance]\ncustom_palette=true\ncolor_scheme_path=~/themes/custom.colors\n";
    assert_eq!(
        qt::palette_path(tilde, config, home).expect("home palette path"),
        Path::new("/home/test/themes/custom.colors")
    );
    let disabled = "[Appearance]\ncustom_palette=false\ncolor_scheme_path=/unused.conf\n";
    assert!(matches!(
        qt::palette_path(disabled, config, home),
        Err(Error::CustomPaletteDisabled(_))
    ));
    assert!(qt::palette_path("[Appearance]\ncustom_palette=true\n", config, home).is_err());
}

#[test]
fn qt_role_arrays_map_paired_colors_and_preserve_disabled_text() {
    let active = (0..22)
        .map(|index| format!("#ff{:06x}", index * 0x010101))
        .collect::<Vec<_>>()
        .join(", ");
    let disabled = (0..22)
        .map(|index| format!("#ff{:06x}", (index + 40) * 0x010101))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "[ColorScheme]\nactive_colors={active}\ninactive_colors={active}\ndisabled_colors={disabled}\n"
    );
    let palette =
        qt::parse(&source, Path::new("palette.conf"), Mode::Dark).expect("valid Qt role arrays");
    assert_eq!(palette.content, color(9 * 0x010101));
    assert_eq!(palette.foreground, color(6 * 0x010101));
    assert_eq!(palette.sidebar, color(10 * 0x010101));
    assert_eq!(palette.selection, color(12 * 0x010101));
    assert_eq!(palette.selection_foreground, color(13 * 0x010101));
    assert_eq!(palette.muted_foreground, color(46 * 0x010101));
}

#[test]
fn qt_rejects_truncated_and_invalid_state_arrays() {
    assert!(
        qt::parse(
            "[ColorScheme]\nactive_colors=#ffffff\n",
            Path::new("palette.conf"),
            Mode::Light
        )
        .is_err()
    );
    let active = vec!["#ffffff"; 21].join(",");
    let source = format!("[ColorScheme]\nactive_colors={active}\ninactive_colors=invalid\n");
    assert!(qt::parse(&source, Path::new("palette.conf"), Mode::Light).is_err());
}

#[test]
fn kde_groups_are_detected_by_content_not_file_extension() {
    let source = "[Colors:Window]\nBackgroundNormal=10,20,30\nForegroundNormal=210,220,230\n\
                  [Colors:View]\nBackgroundNormal=1,2,3\nForegroundNormal=201,202,203\n\
                  [Colors:Selection]\nBackgroundNormal=40,50,60\nForegroundNormal=240,250,255\n";
    let palette = qt::parse(source, Path::new("not-named-colors.conf"), Mode::Dark)
        .expect("valid KDE basic groups");
    assert_eq!(palette.content, color(0x010203));
    assert_eq!(palette.sidebar, color(0x0a141e));
    assert_eq!(palette.selection_foreground, color(0xf0faff));
    assert!(
        qt::parse(
            &source.replace("1,2,3", "999,2,3"),
            Path::new("invalid.colors"),
            Mode::Dark
        )
        .is_err()
    );
}
