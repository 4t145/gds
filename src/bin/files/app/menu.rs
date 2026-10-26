//! The application menu: the toolbar control and the popup it opens.
//!
//! The button and the keyboard shortcut drive one open flag on the workspace,
//! so `Alt+M` (with `F10` as its compatibility stroke) opens exactly the menu
//! the pointer opens, and can never leave a second one behind.

use super::*;

use gds::ui::{self, Shortcut, ShortcutBadge};
use gds_uim::el;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Selectable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::AlertDialog,
    menu::{PopupMenu, PopupMenuItem},
    popover::{Popover, PopoverState},
};
use gpui_kit::prelude::*;

const MENU_WIDTH: f32 = 260.;
const CONTENT_PADDING: f32 = 24.;
/// The product name shown in the about dialog; it is the same name the window
/// and application identity carry, not a translatable message.
const APP_NAME: &str = "GDS Files";

/// One row of the application menu.
///
/// Settings is a workspace command, so its row runs the same dispatch path as
/// every other control for it and shows the same live binding. About belongs to
/// the menu alone: it has no command of its own, which keeps the navigation
/// actions the single owner of that concern.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MenuEntry {
    Settings,
    Volumes,
    About,
}

impl MenuEntry {
    /// The rows of the application menu, in order.
    const ROWS: [Self; 3] = [Self::Settings, Self::Volumes, Self::About];

    fn label_id(self) -> &'static str {
        match self {
            Self::Settings => Command::Settings.label_id(),
            Self::Volumes => "volumes-manage",
            Self::About => "about",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Settings => IconName::Settings,
            Self::Volumes => IconName::HardDrive,
            Self::About => IconName::Info,
        }
    }

    /// The command whose binding the row shows, if the row has one.
    fn command(self) -> Option<Command> {
        match self {
            Self::Settings => Some(Command::Settings),
            Self::About | Self::Volumes => None,
        }
    }

    /// The row's label and, for a command row, its live key badge.
    fn render(self) -> impl Fn(&mut Window, &mut App) -> Div {
        move |_, cx| {
            let badge = self.command().map(ShortcutBadge::new);
            el! {
                .div { flex; items_center; w_full; min_w: MENU_WIDTH.px; gap: CONTENT_PADDING.px } [
                    .div { flex_1; min_w_0; truncate } [$i18n::text(self.label_id(), cx)],
                    @if (let Some(badge) = badge) $(badge), @end,
                ]
            }
        }
    }

    /// The press that runs the row; the popup closes itself afterwards.
    fn on_click(self, view: WeakEntity<Browser>) -> impl Fn(&ClickEvent, &mut Window, &mut App) {
        move |event, window, cx| match self {
            Self::Settings => {
                Browser::menu_command(view.clone(), Command::Settings)(event, window, cx);
            }
            Self::About => Browser::show_about(view.clone(), window, cx),
            Self::Volumes => {
                let _ = view.update(cx, |browser, cx| {
                    browser.menu_open = false;
                    browser.show_collection(Collection::Volumes, window, cx);
                });
            }
        }
    }
}

/// The menu entity kept for one open session.
///
/// The popover's content builder runs on every frame the menu is open, so the
/// menu is built once and reused: rebuilding it per frame would replace its
/// focus handle and its selection.
#[derive(Default)]
struct MenuState {
    menu: Option<Entity<PopupMenu>>,
}

/// The toolbar control that opens the application menu.
///
/// It carries the corner hint and the tooltip badge every other command control
/// has, and the popover marks it open while the menu is up.
#[derive(IntoElement)]
struct Trigger {
    id: ElementId,
    label: SharedString,
    open: bool,
}

impl Trigger {
    fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            open: false,
        }
    }
}

impl Selectable for Trigger {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl RenderOnce for Trigger {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { id, label, open } = self;
        let shortcut = Shortcut::from(Command::Menu);
        let hint = ui::keytips::anchor(window, cx, id.clone(), shortcut);
        let tooltip = ui::command_tooltip(label.clone(), Command::Menu);
        let button = Button::new(id.clone())
            .ghost()
            .small()
            .size(px(ui::ICON_BUTTON_SIZE))
            .icon(Icon::new(IconName::Menu))
            .accessibility_label(label)
            .selected(open);
        div()
            .id(id)
            .relative()
            .flex_shrink_0()
            .tooltip_show_delay(ui::TOOLTIP_DELAY)
            .tooltip(tooltip)
            .child(button)
            .child(hint)
    }
}

/// The toolbar menu control together with the popup it opens.
///
/// `open` is the workspace's flag, so the pointer and the shortcut go through
/// the same state: a press can only open this popup, and a press while it is up
/// closes it instead of stacking another menu.
pub(super) fn application_menu(
    open: bool,
    view: WeakEntity<Browser>,
    cx: &mut Context<Browser>,
) -> AnyElement {
    let trigger = Trigger::new("application-menu", i18n::text("app-menu", cx));
    let opener = view.clone();
    Popover::new("application-menu-popover")
        .appearance(false)
        .overlay_closable(false)
        .open(open)
        .trigger(trigger)
        .on_open_change(move |open, _, cx| {
            if let Some(view) = opener.upgrade() {
                view.update(cx, |browser, cx| browser.set_menu_open(*open, cx));
            }
        })
        .content(move |_, window, cx| popup(view.clone(), window, cx))
        .into_any_element()
}

impl Browser {
    /// Record whether the application menu is showing.
    pub(super) fn set_menu_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.menu_open == open {
            return;
        }
        self.menu_open = open;
        cx.notify();
    }

    /// Show the about dialog over the file workspace.
    ///
    /// It opens on the next frame, after the menu has left the focus tree, so
    /// the dialog records the browser as its return target and gives focus back
    /// there when it closes.
    pub(super) fn show_about(view: WeakEntity<Self>, window: &mut Window, _: &mut App) {
        window.on_next_frame(move |window, cx| {
            let Some(view) = view.upgrade() else {
                return;
            };
            let focus = view.read(cx).focus.clone();
            focus.focus(window, cx);
            window.open_alert_dialog(cx, about_dialog);
        });
    }
}

/// The about dialog, built from the shared alert-dialog visual.
fn about_dialog(dialog: AlertDialog, _: &mut Window, cx: &mut App) -> AlertDialog {
    el! {
        dialog {
            bg: $(cx.theme().popover); text_color: $(cx.theme().popover_foreground);
            title: $(format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION")));
            description: $(i18n::text("about-description", cx));
            icon: IconName::Info.Icon::new;
            ok_text: $(i18n::text("about-close", cx));
        }
    }
}

/// The open menu, built once per open session.
fn popup(
    view: WeakEntity<Browser>,
    window: &mut Window,
    cx: &mut Context<PopoverState>,
) -> AnyElement {
    let state = window.use_keyed_state(SharedString::from("application-menu:menu"), cx, |_, _| {
        MenuState::default()
    });
    if let Some(menu) = state.read(cx).menu.clone() {
        return menu.into_any_element();
    }
    let build_view = view.clone();
    let menu = PopupMenu::build(window, cx, move |menu, _, _| rows(menu, build_view));
    state.update(cx, |state, _| state.menu = Some(menu.clone()));
    // A controlled popover applies its own focus while mounting. Give the
    // menu focus afterwards so its Escape and arrow handlers own the keys.
    let opening_menu = menu.downgrade();
    let opening_view = view.clone();
    window.on_next_frame(move |window, cx| {
        if opening_view
            .upgrade()
            .is_some_and(|view| view.read(cx).menu_open)
            && let Some(menu) = opening_menu.upgrade()
        {
            menu.focus_handle(cx).focus(window, cx);
        }
    });
    let popover = cx.entity().downgrade();
    window
        .subscribe(&menu, cx, {
            // The state owns the menu, so a strong handle here would close the
            // cycle `state -> menu -> listener -> state`.
            let state = state.downgrade();
            move |_, _: &DismissEvent, window, cx| {
                _ = popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                _ = state.update(cx, |state, _| state.menu = None);
                if let Some(view) = view.upgrade() {
                    view.update(cx, |browser, cx| browser.set_menu_open(false, cx));
                }
            }
        })
        .detach();
    menu.into_any_element()
}

/// Application pages and the about entry.
fn rows(mut menu: PopupMenu, view: WeakEntity<Browser>) -> PopupMenu {
    for entry in MenuEntry::ROWS {
        let render = entry.render();
        let on_click = entry.on_click(view.clone());
        menu = menu.item(
            PopupMenuItem::element(render)
                .icon(entry.icon())
                .on_click(on_click),
        );
    }
    menu
}

#[cfg(test)]
mod tests;
