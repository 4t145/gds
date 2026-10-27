use gpui_kit::{
    AnyView, App, AppContext, Context, Entity, IntoElement, Render, Window, component::Root, div,
    prelude::*,
};

pub(crate) struct Surface {
    view: AnyView,
    notices: Entity<gds::notifications::Overlay>,
}

pub(crate) fn wrap(view: impl Into<AnyView>, window: &mut Window, cx: &mut App) -> Entity<Surface> {
    let view = view.into();
    let notices = gds::notifications::Overlay::for_window(window, cx);
    cx.new(|_| Surface { view, notices })
}

/// Preserve typed content access through the application-wide notification layer.
pub(crate) fn content(root: &Root, cx: &App) -> AnyView {
    match root.view().clone().downcast::<Surface>() {
        Ok(surface) => surface.read(cx).view.clone(),
        Err(view) => view,
    }
}

pub(crate) fn notices(window: &Window, cx: &App) -> Option<Entity<gds::notifications::Overlay>> {
    let root = window.root::<Root>().flatten()?;
    let surface = root.read(cx).view().clone().downcast::<Surface>().ok()?;
    Some(surface.read(cx).notices.clone())
}

impl Surface {
    fn on_focus(
        &mut self,
        _: &gds::actions::FocusNotifications,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.notices
            .update(cx, |notices, cx| notices.focus_current(window, cx));
    }
    fn on_close(
        &mut self,
        _: &gds::actions::CloseNotification,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.notices
            .update(cx, |notices, cx| notices.close_current(window, cx));
    }
    fn on_close_all(
        &mut self,
        _: &gds::actions::CloseAllNotifications,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.notices
            .update(cx, |notices, cx| notices.close_all(window, cx));
    }
}

impl Render for Surface {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .on_action(cx.listener(Self::on_focus))
            .on_action(cx.listener(Self::on_close))
            .on_action(cx.listener(Self::on_close_all))
            .child(self.view.clone())
            .child(self.notices.clone())
    }
}
