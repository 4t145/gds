use super::*;

#[gpui_kit::test]
fn settings_return_to_the_same_workspace_and_keep_the_unsent_draft(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    cx.update_window(fixture.window.into(), |_, window, cx| {
        fixture.view.update(cx, |view, cx| {
            view.composer
                .input
                .update(cx, |input, cx| input.set_value("Unsent draft", window, cx));
            view.open_settings(window, cx);
        });
    })
    .expect("open settings from the workspace");
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows().len(), 1));
    cx.update_window(fixture.window.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("settings").visible());
        assert!(!fixture.view.read(cx).window.presented);
        window.click("settings-return", cx);
    })
    .expect("return from settings");
    cx.run_until_parked();
    cx.update_window(fixture.window.into(), |_, window, cx| {
        window.render_frame(cx);
        let view = fixture.view.read(cx);
        assert!(view.window.presented);
        assert_eq!(
            view.composer.input.read(cx).value().as_ref(),
            "Unsent draft"
        );
        assert!(window.find("conversation-header-row").visible());
    })
    .expect("original workspace remains alive");
    assert!(fixture.commands.try_recv().is_err());
}

#[gpui_kit::test]
fn settings_cycle_pages_without_dispatching_workspace_shortcuts(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    cx.update_window(fixture.window.into(), |_, window, cx| {
        fixture
            .view
            .update(cx, |view, cx| view.open_settings(window, cx));
    })
    .expect("settings request");
    cx.run_until_parked();
    cx.update_window(fixture.window.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press("ctrl-tab", cx);
        window.render_frame(cx);
        assert!(window.find("appearance-composer-enter").visible());
        window.press("ctrl-n", cx);
        window.press("ctrl-shift-tab", cx);
        window.render_frame(cx);
        assert!(window.find("settings-reset").visible());
        assert!(window.try_find("appearance-composer-enter").is_none());
        window.click("settings-keybindings", cx);
        window.render_frame(cx);
        assert!(window.find("settings-keybindings-page").visible());
        window.click("settings-search", cx);
        window.input("archive", cx);
        window.render_frame(cx);
        assert!(window.find("settings-keybindings-page").visible());
        window.press("ctrl-tab", cx);
        window.render_frame(cx);
        assert!(window.find("acp-policy-page").visible());
        window.press("escape", cx);
    })
    .expect("page navigation and return");
    cx.run_until_parked();
    assert!(fixture.commands.try_recv().is_err());
    cx.update(|cx| assert_eq!(cx.windows().len(), 1));
    cx.update_window(fixture.window.into(), |_, _, cx| {
        assert!(fixture.view.read(cx).window.presented)
    })
    .expect("workspace restored");
}
