//! Keyboard tab prefixes and the fixed location pages.
//!
//! `Ctrl+Tab` and `Ctrl+Shift+Tab` arm a prefix instead of switching pages
//! immediately. While it is armed the focused view also carries
//! [`TAB_PREFIX_CONTEXT`], where `H`/`R`/`D`/`V`/`N`/`B` open the fixed location
//! pages and `1`-`9` open an ordinary tab by its strip position. A release of
//! Tab without a target key cycles exactly one page; a target key consumes the
//! prefix, so releasing Tab afterwards neither cycles again nor replays the
//! target.
//!
//! The fixed pages (Home, Recent, Favorites, Volumes, Network, Trash) are not ordinary
//! tabs. Home keeps its own directory state in [`Browser::home_tab`] so the
//! table, its selection and its refresh really read the home directory, and
//! opening a folder from it starts an ordinary tab instead of moving Home.

use super::*;

/// An armed `Ctrl+Tab` prefix.
pub(super) struct TabPrefix {
    backwards: bool,
    /// A Tab press is still down; auto-repeat must not re-arm the prefix, and
    /// its eventual release commits the cycle.
    tab_down: bool,
    /// A target key already ran; releasing Tab must not cycle as well.
    consumed: bool,
}

/// A page key behind the tab prefix.
#[derive(Clone, Copy)]
pub(super) enum Target {
    Home,
    Recent,
    Favorites,
    Volumes,
    Network,
    Trash,
    /// Zero-based strip position of an ordinary tab.
    Tab(usize),
}

macro_rules! prefix_handlers {
    ($(($method:ident, $action:ident, $target:expr)),* $(,)?) => {
        $(
            pub(super) fn $method(
                &mut self,
                _: &$action,
                window: &mut Window,
                cx: &mut Context<Self>,
            ) {
                self.apply_tab_prefix_target($target, window, cx);
            }
        )*
    };
}

impl Browser {
    /// Arm the prefix and focus the workspace so target keys do not enter an
    /// editor or trigger its bindings. A modal keeps the page unchanged.
    pub(super) fn arm_tab_prefix(
        &mut self,
        backwards: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_open || window.has_active_dialog(cx) {
            return;
        }
        if self.tab_prefix.is_some() {
            // Auto-repeat while Tab is held: the first press already armed it.
            return;
        }
        self.tab_prefix = Some(TabPrefix {
            backwards,
            tab_down: true,
            consumed: false,
        });
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Commit the prefix when Tab is released without a target key.
    pub(super) fn on_tab_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != "tab" {
            return;
        }
        let Some(prefix) = self.tab_prefix.take() else {
            return;
        };
        if !prefix.tab_down {
            return;
        }
        if prefix.consumed || self.settings_open || window.has_active_dialog(cx) {
            cx.notify();
            return;
        }
        self.cycle_tab(prefix.backwards, window, cx);
        cx.notify();
    }

    /// Run one target key and consume the prefix.
    pub(super) fn apply_tab_prefix_target(
        &mut self,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx) {
            return;
        }
        let Some(prefix) = self.tab_prefix.as_mut() else {
            return;
        };
        prefix.consumed = true;
        match target {
            Target::Home => self.show_home(window, cx),
            Target::Recent => self.show_collection(Collection::Recent, window, cx),
            Target::Favorites => self.show_collection(Collection::Favorites, window, cx),
            Target::Volumes => self.show_collection(Collection::Volumes, window, cx),
            Target::Network => self.show_collection(Collection::Network, window, cx),
            Target::Trash => self.open_trash(window, cx),
            Target::Tab(index) => {
                if let Some(id) = self.tabs.get(index).map(|slot| slot.id) {
                    self.switch_tab(id, window, cx);
                }
            }
        }
    }

    /// Mark an armed prefix as used without cycling on its release.
    ///
    /// Page changes from the pointer keep the prefix state until Tab is
    /// released, so a later release cannot cycle a second time, while its
    /// target bindings stop applying because the prefix was taken.
    pub(super) fn dismiss_tab_prefix(&mut self) {
        if let Some(prefix) = &mut self.tab_prefix {
            prefix.consumed = true;
        }
    }

    /// Drop the prefix completely, without committing its cycle.
    pub(super) fn reset_tab_prefix(&mut self) {
        self.tab_prefix = None;
    }

    /// Show the fixed Home page over its own directory state.
    pub(super) fn show_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(slot) = self.active_mut() {
            slot.save_scroll();
        }
        self.collection = None;
        self.active_id = HOME_TAB_ID;
        self.settings_open = false;
        self.path_editing = false;
        self.sync_filter_input(window, cx);
        self.refresh_if_hidden_changed(cx);
        if self.home_tab.tab.is_none() && !self.home_tab.initial_loading {
            self.load_initial(HOME_TAB_ID, cx);
        }
        self.dismiss_tab_prefix();
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Show one of the fixed collection pages.
    pub(super) fn show_collection(
        &mut self,
        collection: Collection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(slot) = self.active_mut() {
            slot.save_scroll();
        }
        self.settings_open = false;
        self.path_editing = false;
        self.focus.focus(window, cx);
        self.dismiss_tab_prefix();
        match collection {
            Collection::Trash => self.open_trash(window, cx),
            Collection::Recent
            | Collection::Favorites
            | Collection::Network
            | Collection::Volumes => {
                self.collection = Some(collection);
                if collection == Collection::Network && !self.network.loaded {
                    self.refresh_network(cx);
                }
                cx.notify();
            }
        }
    }

    /// Which location row is the visible page.
    pub(super) fn location_selected(&self, target: LocationTarget) -> bool {
        match target {
            LocationTarget::Home => self.home_active(),
            LocationTarget::Recent => self.collection == Some(Collection::Recent),
            LocationTarget::Favorites => self.collection == Some(Collection::Favorites),
            LocationTarget::Volumes => self.collection == Some(Collection::Volumes),
            LocationTarget::Network => self.collection == Some(Collection::Network),
            LocationTarget::Trash => self.collection == Some(Collection::Trash),
        }
    }

    /// Toggle a tab row's hover state so its close control follows the pointer.
    pub(super) fn hover_tab(id: u64) -> impl Fn(&mut Self, &bool, &mut Window, &mut Context<Self>) {
        move |this, hovered, _, cx| {
            let next = if *hovered {
                Some(id)
            } else if this.hovered_tab == Some(id) {
                None
            } else {
                return;
            };
            if this.hovered_tab != next {
                this.hovered_tab = next;
                cx.notify();
            }
        }
    }

    /// Clear the Recent list; only the Recent page runs this.
    ///
    /// The Recent cursor names a record that no longer exists afterwards, so it
    /// is dropped with the list. Favorites and pinned folders are untouched.
    pub(super) fn clear_recent(&mut self, cx: &mut Context<Self>) {
        if self.collection != Some(Collection::Recent) {
            return;
        }
        self.places.recent.clear();
        self.recent_cursor.path = None;
        self.save_places(cx);
        cx.notify();
    }

    prefix_handlers!(
        (on_tab_prefix_home, TabPrefixHome, Target::Home),
        (on_tab_prefix_recent, TabPrefixRecent, Target::Recent),
        (
            on_tab_prefix_favorites,
            TabPrefixFavorites,
            Target::Favorites
        ),
        (on_tab_prefix_network, TabPrefixNetwork, Target::Network),
        (on_tab_prefix_volumes, TabPrefixVolumes, Target::Volumes),
        (on_tab_prefix_trash, TabPrefixTrash, Target::Trash),
        (on_tab_prefix_1, TabPrefix1, Target::Tab(0)),
        (on_tab_prefix_2, TabPrefix2, Target::Tab(1)),
        (on_tab_prefix_3, TabPrefix3, Target::Tab(2)),
        (on_tab_prefix_4, TabPrefix4, Target::Tab(3)),
        (on_tab_prefix_5, TabPrefix5, Target::Tab(4)),
        (on_tab_prefix_6, TabPrefix6, Target::Tab(5)),
        (on_tab_prefix_7, TabPrefix7, Target::Tab(6)),
        (on_tab_prefix_8, TabPrefix8, Target::Tab(7)),
        (on_tab_prefix_9, TabPrefix9, Target::Tab(8)),
    );
}

/// The binding staged on a location row.
///
/// The key only runs behind the prefix, so the row's hint shows the path to it
/// (`Tab+H`) while the plain layer is up and just the target (`H`) once the
/// prefix is armed.
pub(super) fn location_shortcut(target: LocationTarget) -> ui::Shortcut {
    let action: Box<dyn Action> = match target {
        LocationTarget::Home => Box::new(TabPrefixHome),
        LocationTarget::Recent => Box::new(TabPrefixRecent),
        LocationTarget::Favorites => Box::new(TabPrefixFavorites),
        LocationTarget::Volumes => Box::new(TabPrefixVolumes),
        LocationTarget::Network => Box::new(TabPrefixNetwork),
        LocationTarget::Trash => Box::new(TabPrefixTrash),
    };
    ui::Shortcut::new(action, Some(TAB_PREFIX_CONTEXT)).via_prefix(Command::NextTab)
}

/// The ordinal binding staged on the `position + 1`-th ordinary tab row.
///
/// The strip is the cycle order, pinned tabs included; rows past the ninth have
/// no key and therefore no hint.
pub(super) fn ordinal_shortcut(position: usize) -> Option<ui::Shortcut> {
    let action: Box<dyn Action> = match position {
        0 => Box::new(TabPrefix1),
        1 => Box::new(TabPrefix2),
        2 => Box::new(TabPrefix3),
        3 => Box::new(TabPrefix4),
        4 => Box::new(TabPrefix5),
        5 => Box::new(TabPrefix6),
        6 => Box::new(TabPrefix7),
        7 => Box::new(TabPrefix8),
        8 => Box::new(TabPrefix9),
        _ => return None,
    };
    Some(ui::Shortcut::new(action, Some(TAB_PREFIX_CONTEXT)).via_prefix(Command::NextTab))
}
