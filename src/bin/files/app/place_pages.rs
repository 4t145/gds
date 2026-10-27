//! Table pages and selection actions for recent items, favorites and network folders.

mod network;
pub(super) mod table;
pub(super) use network::State as Network;

use super::*;
use gds::ui::{ContentPage, DataListStyle};
use gds_uim::el;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Sizable, table::DataTable};
use gpui_kit::prelude::*;

impl Browser {
    pub(super) fn place_table(
        &self,
        collection: Collection,
    ) -> Option<&Entity<gpui_kit::component::table::TableState<table::LocationTable>>> {
        match collection {
            Collection::Recent => Some(&self.recent_table),
            Collection::Favorites => Some(&self.favorites_table),
            Collection::Network => Some(&self.network_table),
            _ => None,
        }
    }

    pub(super) fn place_paths(&self, collection: Collection) -> Option<Vec<PathBuf>> {
        match collection {
            Collection::Favorites => {
                Some(self.places.favorites.iter().map(PathBuf::from).collect())
            }
            Collection::Recent => Some(self.places.recent.iter().map(PathBuf::from).collect()),
            Collection::Network => Some(self.network.paths.clone()),
            _ => None,
        }
    }

    pub(super) fn select_place(
        &mut self,
        collection: Collection,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.collection != Some(collection)
            || !self
                .place_paths(collection)
                .is_some_and(|paths| paths.contains(&path))
        {
            return;
        }
        if let Some(cursor) = self.place_cursor_mut(collection) {
            cursor.path = Some(path);
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub(super) fn favorite_selected_place(&mut self, remove_only: bool, cx: &mut Context<Self>) {
        let Some(path) = self.selected_place_path() else {
            return;
        };
        if remove_only && !self.places.is_favorite(&path) {
            return;
        }
        self.places.toggle_favorite(&path);
        if self.collection == Some(Collection::Favorites) && !self.places.is_favorite(&path) {
            self.favorites_cursor.path = None;
        }
        self.save_places(cx);
        cx.notify();
    }

    pub(super) fn refresh_network(&mut self, cx: &mut Context<Self>) {
        if self.network.loading {
            return;
        }
        self.network.loading = true;
        self.network.error = None;
        let work = cx.background_executor().spawn(async { network::read() });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |browser, cx| {
                browser.network.loading = false;
                browser.network.loaded = true;
                match result {
                    Ok(paths) => {
                        if browser
                            .network_cursor
                            .path
                            .as_ref()
                            .is_some_and(|path| !paths.contains(path))
                        {
                            browser.network_cursor.path = None;
                        }
                        browser.network.paths = paths;
                    }
                    Err(error) => browser.network.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn refresh_favorites(&mut self, cx: &mut Context<Self>) {
        if self.favorites_loading {
            return;
        }
        self.favorites_loading = true;
        let previous = self.places.favorites.clone();
        let work = cx.background_executor().spawn(async { Places::load() });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |browser, cx| {
                browser.favorites_loading = false;
                match result {
                    Ok(places) if browser.places.favorites == previous => {
                        browser.places.favorites = places.favorites;
                        if browser
                            .favorites_cursor
                            .path
                            .as_ref()
                            .is_some_and(|path| !browser.places.is_favorite(path))
                        {
                            browser.favorites_cursor.path = None;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => notifications::error(error.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn render_places_page(
        &self,
        collection: Collection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let paths = self.place_paths(collection).unwrap_or_default();
        let selected = self.selected_place_path();
        let has_selection = selected.is_some();
        let favorite = selected
            .as_ref()
            .is_some_and(|path| self.places.is_favorite(path));
        let loading = if collection == Collection::Network {
            self.network.loading
        } else {
            self.favorites_loading
        };
        let table_state = self
            .place_table(collection)
            .expect("folder collection has a table");
        let width =
            f32::from(window.bounds().size.width) - self.width() - ContentPage::TABLE_INSET * 2.;
        table::LocationTable::sync(table_state, paths.clone(), selected, width, cx);
        let table = el! {
            ($table_state).DataTable::new {
                bordered: false; with_size: $(gpui_kit::component::Size::Size(px(DataListStyle::ROW_HEIGHT)));
                header_row_height: DataListStyle::HEADER_HEIGHT.px; into_any_element;
            }
        };
        let scroll = table_state
            .read(cx)
            .vertical_scroll_handle
            .0
            .borrow()
            .base_handle
            .clone();
        let area = ContentPage::table_area("locations-table-area", table);
        let list = ui::ListSurface::new(
            "locations-selection-surface",
            area.into_any_element(),
            paths.len(),
            scroll,
        )
        .inset_x(px(ContentPage::TABLE_INSET))
        .on_blank_press(cx.listener(Self::on_list_background_press));
        let on_copy_path = cx.listener(Self::click_command(Command::CopyPaths));
        let mut actions = Vec::new();
        if collection != Collection::Recent {
            let on_copy = cx.listener(Self::click_command(Command::Copy));
            actions.push(el! {
                "place-copy".ui::IconButton::new { icon: IconName::Copy; label: $i18n::text("copy-files", cx);
                    command: Command::Copy; disabled: $(!has_selection); on_click: on_copy;
                }
            }.into_any_element());
        }
        actions.push(el! {
            "place-copy-path".ui::IconButton::new { icon: IconName::ClipboardCopy; label: $i18n::text(Command::CopyPaths.label_id(), cx);
                command: Command::CopyPaths; disabled: $(!has_selection); on_click: on_copy_path;
            }
        }.into_any_element());
        if collection == Collection::Recent {
            let on_clear = cx.listener(Self::click_command(Command::ClearRecent));
            actions.push(el! {
                "clear-recent".ui::IconButton::new { icon: IconName::Broom; label: $i18n::text("clear-recent", cx);
                    command: Command::ClearRecent; disabled: $(paths.is_empty()); on_click: on_clear;
                }
            }.into_any_element());
        } else {
            let on_favorite = cx.listener(Self::click_command(Command::Favorite));
            let on_refresh = cx.listener(Self::click_command(Command::Refresh));
            actions.extend([
                el! {
                    "place-favorite".ui::IconButton::new { icon: $(if favorite { IconName::StarOff } else { IconName::Star });
                        label: $i18n::text(if collection == Collection::Favorites || favorite { "place-favorite-remove" } else { "place-favorite-add" }, cx);
                        command: Command::Favorite; disabled: $(!has_selection); on_click: on_favorite;
                    }
                }.into_any_element(),
                el! {
                    "place-refresh".ui::IconButton::new { icon: IconName::RefreshCw; label: $i18n::text("refresh", cx);
                        command: Command::Refresh; disabled: loading; on_click: on_refresh;
                    }
                }.into_any_element(),
            ]);
        }
        let notice = if collection == Collection::Network {
            if let Some(error) = self.network.error.clone() {
                Some(el! { .div { text_xs; text_color: $cx.theme().danger; whitespace_normal } [error] }.into_any_element())
            } else if loading && !paths.is_empty() {
                Some(el! { .div { text_xs; text_color: $cx.theme().muted_foreground } [$i18n::text("network-loading", cx)] }.into_any_element())
            } else {
                None
            }
        } else {
            None
        };
        ContentPage::new(
            collection.label_id(),
            i18n::text(collection.label_id(), cx),
            list,
        )
        .actions(actions)
        .notice(notice)
        .into_any_element()
    }
}
