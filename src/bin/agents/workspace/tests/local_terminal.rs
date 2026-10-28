use super::*;

#[gpui_kit::test]
fn remote_terminal_toggle_expands_without_spawning_or_falling_back(cx: &mut TestAppContext) {
    let fixture = fixture(cx);
    cx.update_window(fixture.window.into(), |_, window, cx| {
        fixture.view.update(cx, |view, cx| {
            view.window.compact = true;
            view.toggle_terminal(window, cx);
            assert!(!view.window.compact);
            assert!(view.terminals.visible);
            assert!(
                view.terminal_panel().is_none(),
                "fixture uses loopback TCP, not local IPC"
            );
            assert!(!view.has_running_terminals(cx));
        });
    })
    .expect("remote terminal is unavailable");
}

#[cfg(unix)]
#[gpui_kit::test]
fn render_sync_defers_startup_and_hiding_cancels_it(cx: &mut TestAppContext) {
    let fixture = fixture(cx);
    let panel = cx
        .update_window(fixture.window.into(), |_, window, cx| {
            fixture.view.update(cx, |view, cx| {
                view.backend.server =
                    bas::service::Target::Unix("/tmp/terminal-test.sock".into()).into();
                view.navigation.selected_directory = Some(std::env::temp_dir());
                view.terminals.visible = true;
                view.sync_terminal_panel(window, cx);
                let panel = view.terminal_panel().expect("local owner discovered");
                assert!(
                    !panel.read(cx).has_sessions(),
                    "render sync never starts a process"
                );
                view.terminals.visible = false;
                panel
            })
        })
        .expect("sync local panel");
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            !panel.read(cx).has_sessions(),
            "hidden before deferred startup"
        )
    });
}

#[cfg(unix)]
#[gpui_kit::test]
async fn panels_start_on_reveal_and_survive_hiding_and_directory_switches(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let fixture = fixture(cx);
    let shutdowns = cx
        .update_window(fixture.window.into(), |_, window, cx| {
            fixture.view.update(cx, |view, cx| {
                view.backend.server =
                    bas::service::Target::Unix("/tmp/terminal-test.sock".into()).into();
                view.navigation.selected_directory = Some("/tmp/first".into());
                view.toggle_terminal(window, cx);
                let first = view.terminal_panel().expect("explicit local cwd panel");
                assert!(first.read(cx).has_sessions());
                assert!(view.terminal_focused(window, cx));
                view.toggle_terminal(window, cx);
                view.prepare_to_close(window, cx);
                view.set_presented(false, window, cx);
                assert_eq!(
                    view.terminal_panel().expect("retained").entity_id(),
                    first.entity_id()
                );
                view.set_presented(true, window, cx);
                view.navigation.selected_directory = Some("/tmp/second".into());
                view.toggle_terminal(window, cx);
                let second = view.terminal_panel().expect("second cwd panel");
                assert_ne!(first.entity_id(), second.entity_id());
                view.navigation.selected_directory = Some("/tmp/first".into());
                assert_eq!(
                    view.terminal_panel().expect("original").entity_id(),
                    first.entity_id()
                );
                let no_cwd = session();
                view.selected.id = Some(no_cwd.id);
                view.sessions.entries.push(no_cwd);
                assert!(
                    view.terminal_panel().is_none(),
                    "session without cwd must not reuse selected directory"
                );
                view.selected.id = None;
                view.navigation.selected_directory = None;
                assert!(view.terminal_panel().is_none());
                view.shutdown_terminals(cx)
            })
        })
        .expect("panels are workspace-owned");
    for shutdown in shutdowns {
        shutdown.await;
    }
}

#[cfg(unix)]
#[gpui_kit::test]
async fn terminal_tabs_keep_workspace_shortcuts_out_and_f6_restores_composer(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let fixture = fixture(cx);
    let shutdown = cx
        .update_window(fixture.window.into(), |_, window, cx| {
            let shutdown = fixture.view.update(cx, |view, cx| {
                view.backend.server =
                    bas::service::Target::Unix("/tmp/terminal-test.sock".into()).into();
                view.navigation.selected_directory = Some("/tmp".into());
                view.toggle_terminal(window, cx);
                view.terminal_panel()
                    .expect("panel")
                    .update(cx, |panel, cx| {
                        panel.set_cwd(None, cx);
                        let shutdown = panel.shutdown(cx);
                        panel.focus(window, cx);
                        shutdown
                    })
            });
            window.render_frame(cx);
            let epoch = fixture.view.read(cx).selected.epoch;
            for key in [
                "ctrl-n",
                "ctrl-w",
                "ctrl-d",
                "ctrl-l",
                "ctrl-tab",
                "ctrl-q",
                "ctrl-shift-l",
                "pageup",
                "escape",
            ] {
                window.press(key, cx);
            }
            assert_eq!(fixture.view.read(cx).selected.epoch, epoch);
            assert!(!fixture.view.read(cx).window.compact);
            assert!(fixture.view.read(cx).terminal_focused(window, cx));
            window.press("f6", cx);
            shutdown
        })
        .expect("terminal keyboard boundary");
    shutdown.await;
    cx.run_until_parked();
    cx.update_window(fixture.window.into(), |_, window, cx| {
        assert!(
            fixture
                .view
                .read(cx)
                .composer
                .input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.press("ctrl-`", cx);
        assert!(!fixture.view.read(cx).terminals.visible);
    })
    .expect("focus exit and window toggle");
}

#[cfg(unix)]
#[gpui_kit::test]
async fn close_without_confirmation_reaps_hidden_starting_terminal(cx: &mut TestAppContext) {
    // The PTY supervisor is an OS thread, outside the deterministic UI scheduler.
    cx.executor().allow_parking();
    let fixture = fixture(cx);
    let panel = cx
        .update_window(fixture.window.into(), |_, window, cx| {
            fixture.view.update(cx, |view, cx| {
                view.backend.server =
                    bas::service::Target::Unix("/tmp/terminal-test.sock".into()).into();
                view.navigation.selected_directory = Some(std::env::temp_dir());
                view.toggle_terminal(window, cx);
                let panel = view.terminal_panel().expect("local panel");
                panel.update(cx, |panel, cx| panel.open(std::env::temp_dir(), window, cx));
                view.toggle_terminal(window, cx);
                assert!(view.has_running_terminals(cx), "starting counts as running");
                panel
            })
        })
        .expect("request close with hidden terminal");
    let (closed, finished) = futures::channel::oneshot::channel();
    let mut closed = Some(closed);
    let expected = fixture.window.window_id();
    let _closed = cx.update(|cx| {
        cx.on_window_closed(move |_, id| {
            if id == expected
                && let Some(closed) = closed.take()
            {
                let _ = closed.send(());
            }
        })
    });
    cx.update_window(fixture.window.into(), |_, window, cx| {
        fixture
            .view
            .update(cx, |view, cx| view.close_window(window, cx));
        assert!(
            cx.windows()
                .iter()
                .all(|handle| handle.window_id() == expected),
            "close must not open a confirmation window"
        );
    })
    .expect("request immediate close");
    finished
        .await
        .expect("Host closes only after the real PTY shutdown completes");
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!panel.read(cx).has_sessions());
        assert!(
            !cx.windows()
                .iter()
                .any(|window| window.window_id() == fixture.window.window_id())
        );
    });
}
