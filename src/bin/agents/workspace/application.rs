use super::*;
use gds_uim::el;
use gpui_kit::component::menu::PopupMenuItem;

fn application_item(
    label: &'static str,
    action: Option<Box<dyn gpui_kit::Action>>,
) -> PopupMenuItem {
    action_item(label, action, None)
}

pub(super) fn action_item(
    label: &'static str,
    action: Option<Box<dyn gpui_kit::Action>>,
    context: Option<&'static str>,
) -> PopupMenuItem {
    let hint_action = action.as_ref().map(|action| action.boxed_clone());
    let item = PopupMenuItem::element(move |window, _cx| {
        let shortcut = hint_action.as_ref().and_then(|action| {
            let shortcut = gds::ui::Shortcut::new(action.boxed_clone(), context);
            shortcut.binding(window).map(|binding| {
                el! {
                    .div { # "shortcut"; test_support; flex_shrink_0 } [
                        $(gds::ui::ShortcutBadge::binding(binding)),
                    ]
                }
            })
        });
        el! {
            .div {
                id: label;
                test_support;
                role: gpui_kit::Role::Label;
                aria_label: label;
                flex;
                flex_1;
                items_center;
                gap_4;
                text_size: crate::ui::SMALL;
            } [
                .div {
                    id: "label";
                    test_support;
                    flex_1;
                } [
                    $(label),
                ],
                ..shortcut,
            ]
        }
    });
    match action {
        Some(action) => item.action(action),
        None => item,
    }
}

impl Workspace {
    pub(super) fn application_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = cx.entity().downgrade();
        let about = settings.clone();
        self.menu_button(
            layout::icon_button("app-menu", IconName::Menu)
                .accessibility_label(crate::i18n::tr!("app-menu"))
                .tooltip(crate::i18n::tr!("app-menu")),
            2,
            false,
            move |menu, window, _| {
                let parent = window.window_handle();
                let settings = settings.clone();
                let about = about.clone();
                menu.min_w(px(240.))
                    .item(
                        application_item(
                            crate::i18n::tr!("settings-title"),
                            Some(Box::new(crate::application::OpenSettings)),
                        )
                        .on_click(move |_, window, cx| {
                            menus::in_parent(parent, window, cx, |window, cx| {
                                let _ =
                                    settings.update(cx, |this, cx| this.open_settings(window, cx));
                            });
                        }),
                    )
                    .item(
                        application_item(crate::i18n::tr!("app-about"), None).on_click(
                            move |_, window, cx| {
                                menus::in_parent(parent, window, cx, |window, cx| {
                                    let _ = about.update(cx, |this, cx| {
                                        this.open_about_from_menu(window, cx)
                                    });
                                });
                            },
                        ),
                    )
            },
            cx,
        )
    }

    fn open_about_from_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_menu(window, cx);
        if let Err(error) = crate::application::open_about(cx) {
            self.selected.error = Some(crate::i18n::tr!(
                "app-about-error",
                error = (error).to_string()
            ));
            cx.notify();
        }
    }
}
