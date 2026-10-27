use std::collections::HashMap;

use bas::service::tls::ConnectionOptions;
use gpui_kit::{
    App, AppContext, ClickEvent, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding,
    Pixels, Render, Size, Subscription, TestSupportExt, Window, div, prelude::*, px, size,
};

use crate::workspace::Workspace;
use gpui_kit::component::WindowExt;
mod page;
use page::{Content, Page};

gpui_kit::actions!(workspace_pages, [Back]);

pub(crate) fn bind_page_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Back, Some("BasWorkspacePage")),
        KeyBinding::new("alt-left", Back, Some("BasWorkspacePage")),
        KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-w"
            } else {
                "ctrl-w"
            },
            Back,
            Some("BasWorkspacePage"),
        ),
    ]);
}

/// A window owns its backend workspaces; only the selected one is presented.
/// Keeping the others alive preserves drafts and in-flight work without moving it.
pub(crate) struct Host {
    active: ConnectionOptions,
    workspaces: HashMap<ConnectionOptions, Entity<Workspace>>,
    pages: Vec<Page>,
    focus: FocusHandle,
    compact_bounds: Option<Size<Pixels>>,
}

pub(crate) fn workspace_view(
    view: Entity<Workspace>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Host> {
    cx.new(|cx| {
        Host::install_close_handler(window, cx);
        let active = view.read(cx).connection_options().clone();
        Host {
            active: active.clone(),
            workspaces: HashMap::from([(active, view)]),
            pages: Vec::new(),
            focus: cx.focus_handle(),
            compact_bounds: None,
        }
    })
}

impl Host {
    pub(crate) fn active(&self) -> Entity<Workspace> {
        self.workspaces
            .get(&self.active)
            .expect("the active workspace belongs to its window")
            .clone()
    }

    pub(crate) fn show_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<crate::settings::Settings> {
        if let Some(index) = self
            .pages
            .iter()
            .position(|page| matches!(page.content, Content::Settings(_)))
        {
            while self.pages.len() > index + 1 {
                self.back(window, cx);
            }
            let Content::Settings(settings) = &self.pages[index].content else {
                unreachable!()
            };
            let settings = settings.clone();
            settings.update(cx, |settings, cx| settings.on_backend_changed(window, cx));
            settings.read(cx).focus_handle(cx).focus(window, cx);
            return settings;
        }
        crate::connections::manage(self.active.clone(), cx);
        cx.default_global::<crate::agents::Changes>().target =
            Some(window.window_handle().window_id());
        let return_focus = window.focused(cx);
        let settings = cx.new(|cx| crate::settings::Settings::new(window, cx));
        let events = cx.subscribe_in(
            &settings,
            window,
            |host, _, event, window, cx| match event {
                crate::settings::Event::Close => host.back(window, cx),
                crate::settings::Event::CloseWindow => host.close_window(window, cx),
            },
        );
        self.push_page(
            Content::Settings(settings.clone()),
            Some(events),
            return_focus,
            window,
            cx,
        );
        settings.read(cx).focus_handle(cx).focus(window, cx);
        settings
    }

    pub(crate) fn show_catalog(
        &mut self,
        agents: Option<Entity<crate::settings::agents::Agents>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .pages
            .last()
            .is_some_and(|page| matches!(page.content, Content::Catalog(_)))
        {
            return;
        }
        if agents.is_none() {
            crate::connections::manage(self.active.clone(), cx);
        }
        let return_focus = window.focused(cx);
        let content = crate::settings::agents::window::content(agents, window, cx);
        self.push_page(Content::Catalog(content), None, return_focus, window, cx);
    }

    pub(crate) fn show_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .pages
            .last()
            .is_some_and(|page| matches!(page.content, Content::About(_)))
        {
            return;
        }
        let return_focus = window.focused(cx);
        let about = cx.new(|_| crate::application::About);
        self.push_page(Content::About(about), None, return_focus, window, cx);
    }

    pub(crate) fn show_history(
        &mut self,
        request: crate::history::OpenRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let return_focus = window.focused(cx);
        let browser = crate::history::window::content(request, window, cx)?;
        self.push_history(browser, return_focus, window, cx);
        Ok(())
    }

    pub(crate) fn push_history(
        &mut self,
        browser: Entity<crate::history::Browser>,
        return_focus: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.push_page(
            Content::History(browser.clone()),
            None,
            return_focus,
            window,
            cx,
        );
        crate::history::window::focus(&browser, window, cx);
    }

    fn push_page(
        &mut self,
        content: Content,
        events: Option<Subscription>,
        return_focus: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pages.is_empty() {
            let workspace = self.active();
            if workspace.read(cx).is_compact() {
                self.compact_bounds = Some(window.bounds().size);
            }
            workspace.update(cx, |workspace, cx| {
                workspace.set_presented(false, window, cx)
            });
            if self.compact_bounds.is_some() {
                window.resize(size(px(1000.), px(740.)));
            }
        }
        self.pages.push(Page {
            content,
            return_focus,
            _events: events,
        });
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn prepare_page(page: &Page, window: &mut Window, cx: &mut App) {
        match &page.content {
            Content::Settings(settings) => {
                settings.update(cx, |settings, cx| settings.prepare_to_close(window, cx));
                crate::settings::window::unregister(window.window_handle().window_id(), cx);
            }
            Content::Catalog(content) => {
                content.update(cx, |content, cx| content.prepare_to_leave(cx))
            }
            _ => {}
        }
    }

    pub(crate) fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.pages.pop() else {
            return;
        };
        Self::prepare_page(&page, window, cx);
        if self.pages.is_empty() {
            if let Some(bounds) = self.compact_bounds.take() {
                window.resize(bounds);
            }
            self.active().update(cx, |workspace, cx| {
                workspace.set_presented(true, window, cx)
            });
        }
        if let Some(focus) = page.return_focus {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn return_to_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while !self.pages.is_empty() {
            self.back(window, cx);
        }
    }

    pub(crate) fn show_workspace_for(
        &mut self,
        target: ConnectionOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.select(target, window, cx)?;
        self.return_to_workspace(window, cx);
        Ok(())
    }

    fn on_back(&mut self, _: &Back, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if !window.has_active_dialog(cx) {
            self.back(window, cx);
        }
    }

    fn on_back_click(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !window.has_active_dialog(cx) {
            self.back(window, cx);
        }
    }

    fn close_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prepare_to_close(window, cx);
        window.remove_window();
    }

    fn on_close_click(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.close_window(window, cx);
    }

    pub(super) fn select(
        &mut self,
        target: ConnectionOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if self.active == target {
            return Ok(());
        }
        let previous = self.active();
        let compact = previous.read(cx).is_compact();
        let next = match self.workspaces.get(&target) {
            Some(view) => view.clone(),
            None => {
                let launch = cx
                    .try_global::<super::Launch>()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("The workspace runtime is unavailable"))?;
                cx.new(|cx| {
                    Workspace::new(
                        target.clone(),
                        launch.cwd,
                        compact,
                        &launch.runtime,
                        window,
                        cx,
                    )
                })
            }
        };
        previous.update(cx, |view, cx| view.set_presented(false, window, cx));
        self.active = target.clone();
        self.workspaces.insert(target, next.clone());
        if self.pages.is_empty() {
            next.update(cx, |view, cx| view.show_in_window(compact, window, cx));
        } else {
            next.update(cx, |view, cx| view.set_presented(false, window, cx));
        }
        Self::install_close_handler(window, cx);
        cx.notify();
        Ok(())
    }

    fn install_close_handler(window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = owner.update(cx, |host, cx| {
                host.prepare_to_close(window, cx);
            });
            true
        });
    }

    fn prepare_to_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for page in &self.pages {
            Self::prepare_page(page, window, cx);
        }
        crate::settings::window::unregister(window.window_handle().window_id(), cx);
        for view in self.workspaces.values() {
            view.update(cx, |view, cx| view.set_presented(false, window, cx));
        }
    }
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(page) = self.pages.last() else {
            return self.active().into_any_element();
        };
        window.set_window_title(page.content.title());
        if matches!(page.content, Content::Settings(_)) {
            return page.content.element();
        }
        let title = page.content.title();
        let id = page.content.id();
        let content = page.content.element();
        let sidebar_width = page.content.sidebar_width();
        let back = gds::ui::IconButton::new("page-return")
            .icon(gpui_kit::assets::IconName::ArrowLeft)
            .label(crate::i18n::tr!("settings-return"))
            .shortcut(gds::ui::Shortcut::new(
                Box::new(Back),
                Some("BasWorkspacePage"),
            ))
            .on_click(cx.listener(Self::on_back_click))
            .into_any_element();
        let controls = div()
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .child(title)
            .when(sidebar_width.is_none(), |controls| {
                controls.child(gds::ui::WindowDragArea)
            })
            .into_any_element();
        let frame = sidebar_width.map_or_else(gds::ui::Frame::without_sidebar, gds::ui::Frame::new);
        let toolbar = frame.toolbar_with_window_controls(
            back,
            controls,
            gds::ui::window::WindowControls::new()
                .on_close_window(cx.listener(Self::on_close_click)),
            cx,
        );
        div()
            .id(id)
            .test_support()
            .size_full()
            .flex()
            .flex_col()
            .bg(gds::theme::content_background(cx))
            .key_context("BasWorkspacePage")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::on_back))
            .child(toolbar)
            .child(div().flex_1().min_h_0().child(content))
            .into_any_element()
    }
}
