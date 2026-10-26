//! Keyboard context menus reuse the same captured targets and menu builders
//! as pointer menus. Their position follows the selected row, not the mouse.

use std::{collections::HashSet, ffi::OsString};

use super::*;
use gds_uim::el;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};

const EDGE_MARGIN: f32 = 8.;
const MENU_WIDTH: f32 = 220.;

pub(super) struct OpenMenu {
    menu: Entity<PopupMenu>,
    position: Point<Pixels>,
    _dismiss: Subscription,
}

#[derive(Clone, PartialEq)]
enum Target {
    Files {
        path: PathBuf,
        selected: Vec<PathBuf>,
    },
    Volume(storage::VolumeId),
    Trash {
        id: OsString,
        selected: HashSet<OsString>,
    },
    Place {
        collection: Collection,
        path: PathBuf,
    },
}

struct RowAnchor {
    index: usize,
    height: Pixels,
    scroll: ScrollHandle,
}

struct MenuRequest {
    target: Target,
    position: Point<Pixels>,
    focus: FocusHandle,
}

impl RowAnchor {
    fn position(&self) -> Point<Pixels> {
        let bounds = self.scroll.bounds();
        let bottom = bounds.top() + self.height * (self.index + 1) as f32 + self.scroll.offset().y;
        point(
            bounds.left() + px(ui::DataListStyle::CELL_PADDING_X),
            bottom.clamp(bounds.top(), bounds.bottom()),
        )
    }
}

impl Browser {
    fn selection_menu_target(&self, cx: &App) -> Option<(Target, RowAnchor)> {
        let height = px(ui::DataListStyle::ROW_HEIGHT);
        match self.collection {
            Some(Collection::Volumes) => {
                let id = self.volume_action_target(cx)?;
                let index = self
                    .storage
                    .read(cx)
                    .snapshot
                    .volumes
                    .iter()
                    .filter(|volume| volume.visible)
                    .position(|volume| volume.id == id)?;
                let handle = &self.volume_table.read(cx).vertical_scroll_handle;
                handle.scroll_to_item(index, ScrollStrategy::Nearest);
                Some((
                    Target::Volume(id),
                    RowAnchor {
                        index,
                        height,
                        scroll: handle.0.borrow().base_handle.clone(),
                    },
                ))
            }
            Some(Collection::Trash) => {
                let selected = self.trash.selection();
                let index = self
                    .trash
                    .cursor_index()
                    .filter(|&index| selected.contains(&self.trash.rows()[index].id))
                    .or_else(|| {
                        self.trash
                            .rows()
                            .iter()
                            .position(|entry| selected.contains(&entry.id))
                    })?;
                let id = self.trash.rows()[index].id.clone();
                let handle = self.trash.scroll_handle(cx);
                handle.scroll_to_item(index, ScrollStrategy::Nearest);
                Some((
                    Target::Trash { id, selected },
                    RowAnchor {
                        index,
                        height,
                        scroll: handle.0.borrow().base_handle.clone(),
                    },
                ))
            }
            Some(collection @ (Collection::Recent | Collection::Favorites)) => {
                let path = self.selected_place_path()?;
                let index = collection
                    .place_paths(&self.places)?
                    .iter()
                    .position(|entry| Path::new(entry) == path)?;
                let scroll = self.place_cursor(collection)?.scroll.clone();
                scroll.scroll_to_item(index);
                Some((
                    Target::Place { collection, path },
                    RowAnchor {
                        index,
                        height: px(ui::SIDEBAR_ITEM_HEIGHT),
                        scroll,
                    },
                ))
            }
            None => {
                let slot = self
                    .active()
                    .filter(|slot| slot.files_available() && !slot.is_loading())?;
                let selected = self.selected_visible_paths();
                let path = slot
                    .cursor
                    .as_ref()
                    .filter(|path| selected.contains(path))
                    .or(selected.first())?
                    .clone();
                let index = slot
                    .table_rows
                    .iter()
                    .position(|entry| entry.path == path)?;
                slot.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
                Some((
                    Target::Files { path, selected },
                    RowAnchor {
                        index,
                        height,
                        scroll: slot.scroll.0.borrow().base_handle.clone(),
                    },
                ))
            }
            Some(Collection::Network) => None,
        }
    }

    pub(super) fn open_selection_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.list_keys_available(window, cx) {
            return;
        }
        let Some((target, _)) = self.selection_menu_target(cx) else {
            return;
        };
        let origin = (self.active_id, self.collection);
        let browser = cx.entity().downgrade();
        // Let the selected row scroll into view before positioning its menu.
        window.on_next_frame(move |window, cx| {
            let prepared = browser
                .update(cx, |view, cx| {
                    if !view.list_keys_available(window, cx)
                        || (view.active_id, view.collection) != origin
                    {
                        return None;
                    }
                    let (current, anchor) = view.selection_menu_target(cx)?;
                    (current == target).then(|| (anchor.position(), view.focus.clone()))
                })
                .ok()
                .flatten();
            if let Some((position, focus)) = prepared {
                Self::show_selection_menu(
                    browser,
                    MenuRequest {
                        target,
                        position,
                        focus,
                    },
                    window,
                    cx,
                );
            }
        });
        cx.notify();
    }

    fn show_selection_menu(
        browser: WeakEntity<Self>,
        request: MenuRequest,
        window: &mut Window,
        cx: &mut App,
    ) {
        let MenuRequest {
            target,
            position,
            focus,
        } = request;
        let builder = browser.clone();
        let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
            let menu = menu.action_context(focus);
            let Some(view) = builder.upgrade() else {
                return menu;
            };
            view.update(cx, |view, cx| match target {
                Target::Files { path, .. } => view.entry_context_menu(path, menu, window, cx),
                Target::Volume(id) => view.build_volume_menu(builder.clone(), &id, menu, cx),
                Target::Trash { .. } => view.trash_selection_menu(menu, cx),
                Target::Place { .. } => view.place_selection_menu(menu, cx),
            })
        });
        if menu.read(cx).is_empty() {
            return;
        }
        let menu_id = menu.entity_id();
        let _ = browser.update(cx, |view, cx| {
            let dismiss =
                cx.subscribe_in(&menu, window, move |view, _, _: &DismissEvent, _, cx| {
                    if view
                        .keyboard_menu
                        .as_ref()
                        .is_some_and(|open| open.menu.entity_id() == menu_id)
                    {
                        view.keyboard_menu = None;
                        cx.notify();
                    }
                });
            view.keyboard_menu = Some(OpenMenu {
                menu: menu.clone(),
                position,
                _dismiss: dismiss,
            });
            cx.notify();
        });
        menu.focus_handle(cx).focus(window, cx);
        // The new menu enters the focus tree on this frame.
        let opening = menu.downgrade();
        window.on_next_frame(move |window, cx| {
            if browser.upgrade().is_some_and(|view| {
                view.read(cx)
                    .keyboard_menu
                    .as_ref()
                    .is_some_and(|open| open.menu.entity_id() == menu_id)
            }) && let Some(menu) = opening.upgrade()
            {
                menu.focus_handle(cx).focus(window, cx);
                window.dispatch_action(Box::new(gpui_kit::base::actions::SelectDown), cx);
            }
        });
    }

    pub(super) fn render_selection_menu(&self, window: &Window) -> Option<AnyElement> {
        let open = self.keyboard_menu.as_ref()?;
        Some(
            deferred(
                anchored().child(
                    div()
                        .w(window.bounds().size.width)
                        .h(window.bounds().size.height)
                        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                        .child(
                            anchored()
                                .position(open.position)
                                .snap_to_window_with_margin(px(EDGE_MARGIN))
                                .anchor(Anchor::TopLeft)
                                .child(open.menu.clone()),
                        ),
                ),
            )
            .with_priority(gpui_kit::base::POPUP_PRIORITY)
            .into_any_element(),
        )
    }

    fn place_selection_menu(&self, mut menu: PopupMenu, cx: &Context<Self>) -> PopupMenu {
        let Some(path) = self.selected_place_path() else {
            return menu;
        };
        for command in [Command::Open, Command::OpenInNewTab, Command::CopyPaths] {
            let browser = cx.entity().downgrade();
            let target = path.clone();
            let label = i18n::text(command.label_id(), cx);
            let shortcut = command != Command::OpenInNewTab || self.picker.is_none();
            let row = move |_: &mut Window, _: &mut App| {
                el! {
                    .div { flex; items_center; w_full; min_w: MENU_WIDTH.px; gap: EDGE_MARGIN.px } [
                        .div { flex_1; min_w_0 } [$(label.clone())],
                        @if (shortcut) command.ui::ShortcutBadge::new, @end,
                    ]
                }
            };
            menu = menu.item(PopupMenuItem::element(row).on_click(move |_, window, cx| {
                let _ = browser.update(cx, |view, cx| match command {
                    Command::Open => {
                        view.collection = None;
                        view.activate_path(target.clone(), cx);
                    }
                    Command::OpenInNewTab => view.open_tab(target.clone(), false, window, cx),
                    Command::CopyPaths => view.copy_selected_paths(&[target.clone()], cx),
                    _ => unreachable!("place menus only contain place commands"),
                });
            }));
        }
        menu
    }

    pub(super) fn place_menu(
        browser: WeakEntity<Self>,
        collection: Collection,
        path: PathBuf,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu {
        move |menu, window, cx| {
            let Some(view) = browser.upgrade() else {
                return menu;
            };
            view.update(cx, |view, cx| {
                if view.collection != Some(collection) {
                    return menu;
                }
                let listed = collection
                    .place_paths(&view.places)
                    .is_some_and(|paths| paths.iter().any(|entry| Path::new(entry) == path));
                if !listed {
                    return menu;
                }
                if let Some(cursor) = view.place_cursor_mut(collection) {
                    cursor.path = Some(path.clone());
                }
                view.focus.focus(window, cx);
                cx.notify();
                view.place_selection_menu(menu, cx)
            })
        }
    }
}
