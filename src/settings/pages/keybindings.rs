use gds_uim::el;
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

use crate::{
    actions::Command,
    i18n::text,
    settings::{
        View,
        controls::{SettingNote, SettingRow},
    },
    ui::{ShortcutBadge, dialog::DialogCommand},
};

/// Trash actions are bound globally but only run on the Trash page; listing
/// them apart keeps every row's key honest.
const TRASH_COMMANDS: [Command; 3] = [
    Command::RestoreSelection,
    Command::PurgeSelection,
    Command::EmptyTrash,
];
/// Actions that belong to one location page rather than the file workspace.
const PAGE_COMMANDS: [Command; 1] = [Command::ClearRecent];

impl View {
    pub(super) fn render_keybindings(&self, cx: &App) -> Div {
        let note = SettingNote::new(text("settings-shortcuts-note", cx));
        let prefix = SettingNote::new(text("settings-tab-prefix", cx));
        let rows: Vec<_> = Command::ALL
            .into_iter()
            .filter(|command| !TRASH_COMMANDS.contains(command) && !PAGE_COMMANDS.contains(command))
            .map(|command| {
                SettingRow::new(text(command.label_id(), cx), ShortcutBadge::new(command))
            })
            .collect();
        let page_heading = section_heading("settings-page-actions", cx);
        let page_rows: Vec<_> = PAGE_COMMANDS
            .into_iter()
            .map(|command| {
                SettingRow::new(text(command.label_id(), cx), ShortcutBadge::new(command))
            })
            .collect();
        let trash_heading = section_heading("settings-trash-actions", cx);
        let trash_note = SettingNote::new(text("settings-trash-note", cx));
        let trash_rows: Vec<_> = TRASH_COMMANDS
            .into_iter()
            .map(|command| {
                SettingRow::new(text(command.label_id(), cx), ShortcutBadge::new(command))
            })
            .collect();
        let dialog_heading_label = text("settings-dialogs", cx);
        let dialog_heading = el! {
            .div { pt_6; pb_2; text_xs; text_color: $(cx.theme().muted_foreground) }
            [dialog_heading_label]
        };
        let dialog_note = SettingNote::new(text("settings-dialogs-note", cx));
        let dialog_rows: Vec<_> = DialogCommand::ALL
            .into_iter()
            .map(|command| {
                SettingRow::new(
                    text(command.label_id(), cx),
                    ShortcutBadge::bound(command.shortcut()),
                )
            })
            .collect();
        el! {
            .div [
                note, ..rows,
                prefix,
                page_heading, ..page_rows,
                trash_heading, trash_note, ..trash_rows,
                dialog_heading, dialog_note, ..dialog_rows,
            ]
        }
    }
}

fn section_heading(id: &'static str, cx: &App) -> Div {
    let label = text(id, cx);
    el! {
        .div { pt_6; pb_2; text_xs; text_color: $(cx.theme().muted_foreground) }
        [label]
    }
}
