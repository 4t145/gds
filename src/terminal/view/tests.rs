use super::View;
use crate::terminal::{
    Preferences,
    session::{Session, SpawnOptions},
};
use gpui_kit::{
    AppContext, Context, Entity, FocusHandle, IntoElement, KeyBinding, Render, TestAppContext,
    Window, actions, component::Root, div, prelude::*, px, size, test::TestWindowExt,
};

actions!(
    terminal_focus_test,
    [WorkspaceOnly, Settings, Toggle, ReturnToHost]
);
struct Host {
    view: Entity<View>,
    focus: FocusHandle,
    workspace_actions: usize,
    settings: usize,
    toggles: usize,
    exits: usize,
}
impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .key_context("TerminalFocusTest")
            .track_focus(&self.focus)
            .on_action(cx.listener(|host, _: &WorkspaceOnly, _, _| host.workspace_actions += 1))
            .on_action(cx.listener(|host, _: &Settings, _, _| host.settings += 1))
            .on_action(cx.listener(|host, _: &Toggle, _, _| host.toggles += 1))
            .on_action(cx.listener(|host, _: &ReturnToHost, _, _| host.exits += 1))
            .child(self.view.clone())
    }
}

#[gpui_kit::test]
fn single_terminal_respects_its_hosts_key_map(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::i18n::install(crate::i18n::Language::English, cx).expect("test messages");
        // The host, not the reusable terminal, chooses all these bindings.
        cx.bind_keys([
            KeyBinding::new(
                "ctrl-w",
                WorkspaceOnly,
                Some("TerminalFocusTest && !GdsTerminal"),
            ),
            KeyBinding::new("ctrl-,", Settings, Some("TerminalFocusTest")),
            KeyBinding::new("ctrl-`", Toggle, Some("TerminalFocusTest")),
            KeyBinding::new("f6", ReturnToHost, Some("TerminalFocusTest > GdsTerminal")),
        ]);
    });
    let session = Session::spawn(SpawnOptions {
        cwd: std::env::temp_dir(),
        args: vec!["-c".into(), "exit 0".into()],
        ..SpawnOptions::default()
    })
    .expect("local shell for terminal focus test");
    let mut host = None;
    let window = cx.open_window(size(px(800.), px(480.)), |window, cx| {
        let view = cx.new(|cx| View::new(session.clone(), Preferences::default(), window, cx));
        let entity = cx.new(|cx| Host {
            view,
            focus: cx.focus_handle(),
            workspace_actions: 0,
            settings: 0,
            toggles: 0,
            exits: 0,
        });
        host = Some(entity.clone());
        Root::new(entity, window, cx)
    });
    let host = host.expect("host entity");
    cx.update_window(window.into(), |_, window, cx| {
        host.read(cx).focus.clone().focus(window, cx);
        window.render_frame(cx);
        window.press("ctrl-w", cx);
        assert_eq!(host.read(cx).workspace_actions, 1);
        let inside = host.read(cx).view.read(cx).focus_handle();
        inside.focus(window, cx);
        window.render_frame(cx);
        window.press("ctrl-w", cx);
        window.press("ctrl-,", cx);
        window.press("ctrl-`", cx);
        window.press("f6", cx);
        let host = host.read(cx);
        assert_eq!(
            host.workspace_actions, 1,
            "host close must not steal shell Ctrl+W"
        );
        assert_eq!((host.settings, host.toggles, host.exits), (1, 1, 1));
    })
    .expect("terminal key dispatch");
    session.terminate();
}

#[gpui_kit::test]
fn changing_terminal_typography_updates_existing_grid_metrics(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::i18n::install(crate::i18n::Language::English, cx).expect("test messages");
    });
    let session = Session::spawn(SpawnOptions {
        cwd: std::env::temp_dir(),
        args: vec!["-c".into(), "exit 0".into()],
        ..SpawnOptions::default()
    })
    .expect("local shell for typography test");
    let mut terminal = None;
    let window = cx.open_window(size(px(800.), px(480.)), |window, cx| {
        let view = cx.new(|cx| View::new(session.clone(), Preferences::default(), window, cx));
        terminal = Some(view.clone());
        Root::new(view, window, cx)
    });
    let terminal = terminal.expect("terminal view");
    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        let before = terminal.read(cx).geometry;
        let preferences = Preferences {
            font_family: ".SystemMonospace".into(),
            font_size: 24.,
            ..Preferences::default()
        };
        terminal.update(cx, |view, cx| view.set_preferences(preferences.clone(), cx));
        window.render_frame(cx);
        let view = terminal.read(cx);
        let after = view.geometry;
        assert!(after.cell.width > before.cell.width);
        assert!(after.cell.height > before.cell.height);
        assert!(
            after.bounds.size.width / after.cell.width
                < before.bounds.size.width / before.cell.width,
            "larger text must reduce grid columns"
        );
        assert!(
            after.bounds.size.height / after.cell.height
                < before.bounds.size.height / before.cell.height,
            "larger text must reduce grid rows"
        );
        assert!(view.fonts.as_ref().expect("resolved font").matches(
            &preferences.font_family,
            preferences.font_size,
            f32::from(window.scale_factor()),
        ));
        assert_eq!(view.preferences, preferences);
    })
    .expect("terminal typography update");
    session.terminate();
}
