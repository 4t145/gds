use std::collections::HashMap;

use bas::service::tls::ConnectionOptions;
use gpui_kit::{
    App, AppContext, Context, Entity, Focusable, IntoElement, Pixels, Render, Size, Subscription,
    Window, px, size,
};

use crate::workspace::Workspace;

/// A window owns its backend workspaces; only the selected one is presented.
/// Keeping the others alive preserves drafts and in-flight work without moving it.
pub(crate) struct Host {
    active: ConnectionOptions,
    workspaces: HashMap<ConnectionOptions, Entity<Workspace>>,
    settings: Option<Entity<crate::settings::Settings>>,
    settings_events: Option<Subscription>,
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
            settings: None,
            settings_events: None,
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
        if let Some(settings) = &self.settings {
            settings.update(cx, |settings, cx| settings.on_backend_changed(window, cx));
            settings.read(cx).focus_handle(cx).focus(window, cx);
            return settings.clone();
        }
        crate::connections::manage(self.active.clone(), cx);
        cx.default_global::<crate::agents::Changes>().target =
            Some(window.window_handle().window_id());
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
        let settings = cx.new(|cx| crate::settings::Settings::new(window, cx));
        self.settings_events = Some(cx.subscribe_in(
            &settings,
            window,
            |host, _, event, window, cx| match event {
                crate::settings::Event::Close => host.close_settings(window, cx),
            },
        ));
        settings.read(cx).focus_handle(cx).focus(window, cx);
        self.settings = Some(settings.clone());
        cx.notify();
        settings
    }

    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(settings) = self.settings.take() {
            settings.update(cx, |settings, cx| settings.prepare_to_close(window, cx));
        }
        self.settings_events = None;
        crate::settings::window::unregister(window.window_handle().window_id(), cx);
        if let Some(bounds) = self.compact_bounds.take() {
            window.resize(bounds);
        }
        self.active().update(cx, |workspace, cx| {
            workspace.set_presented(true, window, cx)
        });
        cx.notify();
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
        if self.settings.is_none() {
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
                if let Some(settings) = &host.settings {
                    settings.update(cx, |settings, cx| settings.prepare_to_close(window, cx));
                }
                crate::settings::window::unregister(window.window_handle().window_id(), cx);
                for view in host.workspaces.values() {
                    view.update(cx, |view, cx| view.set_presented(false, window, cx));
                }
            });
            true
        });
    }
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.settings {
            Some(settings) => settings.clone().into_any_element(),
            None => self.active().into_any_element(),
        }
    }
}
