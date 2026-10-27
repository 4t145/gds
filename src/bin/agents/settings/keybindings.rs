use gds::ui::{Shortcut, ShortcutBadge, settings::SettingRow};
use gds_uim::el;
use gpui_kit::{App, IntoElement, TestSupportExt, component::ActiveTheme, div, prelude::*};

pub(super) fn render(query: &str, cx: &App) -> gpui_kit::AnyElement {
    let application = vec![
        (
            crate::i18n::tr!("settings-title"),
            Shortcut::new(Box::new(crate::application::OpenSettings), None),
        ),
        (
            crate::i18n::tr!("app-quit"),
            Shortcut::new(Box::new(crate::application::Quit), None),
        ),
    ];
    let settings = vec![
        (
            crate::i18n::tr!("settings-return"),
            Shortcut::new(Box::new(super::window::CloseWindow), Some("BasSettings")),
        ),
        (
            crate::i18n::tr!("shortcuts-next-page"),
            Shortcut::new(Box::new(super::window::NextPage), Some("BasSettings")),
        ),
        (
            crate::i18n::tr!("shortcuts-previous-page"),
            Shortcut::new(Box::new(super::window::PreviousPage), Some("BasSettings")),
        ),
        (
            crate::i18n::tr!("sidebar-toggle"),
            Shortcut::new(Box::new(super::window::ToggleSidebar), Some("BasSettings")),
        ),
    ];
    let notifications = [
        gds::actions::Command::FocusNotifications,
        gds::actions::Command::CloseNotification,
        gds::actions::Command::CloseAllNotifications,
    ]
    .into_iter()
    .map(|command| (gds::i18n::text(command.label_id(), cx), command.into()))
    .collect();
    let dialogs = gds::ui::dialog::DialogCommand::ALL
        .into_iter()
        .map(|command| (gds::i18n::text(command.label_id(), cx), command.shortcut()))
        .collect();
    let sections = [
        (crate::i18n::tr!("settings-title"), owned(application)),
        (
            crate::i18n::tr!("shortcuts-workspace"),
            owned(crate::workspace::shortcuts::settings_entries()),
        ),
        (crate::i18n::tr!("shortcuts-settings"), owned(settings)),
        (crate::i18n::tr!("shortcuts-notifications"), notifications),
        (crate::i18n::tr!("shortcuts-dialogs"), dialogs),
    ];
    let query = query.trim().to_lowercase();
    let query = if ["shortcuts", "keyboard shortcuts", "keybindings", "快捷键"]
        .contains(&query.as_str())
    {
        String::new()
    } else {
        query
    };
    el! {
        .div { # "settings-keybindings-page"; test_support; size_full; min_h_0; overflow_y_scroll; p_6 } [
            .div { pb_4; text_sm; text_color: $(cx.theme().muted_foreground) } [$(crate::i18n::tr!("shortcuts-note"))],
            ..$(sections.into_iter().filter_map(|(title, entries)| {
                let rows: Vec<_> = entries.into_iter().filter(|(label, _)| {
                    query.is_empty() || title.to_lowercase().contains(&query) || label.to_lowercase().contains(&query)
                }).map(|(label, shortcut)| SettingRow::new(label, ShortcutBadge::bound(shortcut))).collect();
                (!rows.is_empty()).then(|| el! {
                    .div { pb_6 } [
                        .div { pb_2; text_xs; text_color: $(cx.theme().muted_foreground) } [title],
                        ..rows,
                    ]
                })
            })),
        ]
    }.into_any_element()
}

fn owned(entries: Vec<(&'static str, Shortcut)>) -> Vec<(gpui_kit::SharedString, Shortcut)> {
    entries
        .into_iter()
        .map(|(label, shortcut)| (label.into(), shortcut))
        .collect()
}
