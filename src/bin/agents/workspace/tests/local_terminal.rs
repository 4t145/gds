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
async fn render_sync_defers_startup_and_hiding_cancels_it(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
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
        );
        assert!(panel.read(cx).needs_initial_reveal());
    });
    cx.update_window(fixture.window.into(), |_, window, cx| {
        fixture.view.update(cx, |view, cx| {
            view.set_presented(true, window, cx);
            view.terminals.visible = true;
            view.sync_terminal_panel(window, cx);
            assert!(
                !panel.read(cx).has_sessions(),
                "retry still defers process startup"
            );
        });
    })
    .expect("return to canceled owner");
    cx.run_until_parked();
    let shutdown = cx.update(|cx| {
        panel.update(cx, |panel, cx| {
            assert!(
                panel.has_sessions(),
                "returning retries canceled initial reveal"
            );
            assert!(!panel.needs_initial_reveal());
            panel.shutdown(cx)
        })
    });
    shutdown.await;
}

#[cfg(unix)]
#[gpui_kit::test]
async fn panels_start_on_reveal_and_survive_hiding_and_directory_switches(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let directory = tempfile::tempdir().expect("terminal owner directories");
    let first_cwd = directory.path().join("first");
    let second_cwd = directory.path().join("second");
    std::fs::create_dir(&first_cwd).expect("first cwd");
    std::fs::create_dir(&second_cwd).expect("second cwd");
    let fixture = fixture(cx);
    let shutdowns = cx
        .update_window(fixture.window.into(), |_, window, cx| {
            fixture.view.update(cx, |view, cx| {
                view.backend.server =
                    bas::service::Target::Unix("/tmp/terminal-test.sock".into()).into();
                view.navigation.selected_directory = Some(first_cwd.clone());
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
                view.navigation.selected_directory = Some(second_cwd.clone());
                view.toggle_terminal(window, cx);
                let second = view.terminal_panel().expect("second cwd panel");
                assert_ne!(first.entity_id(), second.entity_id());
                view.navigation.selected_directory = Some(first_cwd.clone());
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
async fn terminal_navigation_is_nested_in_its_workspace_and_dock_does_not_split_sidebar(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    let directory = tempfile::tempdir().expect("workspace terminal directories");
    let first_cwd = directory.path().join("first");
    let second_cwd = directory.path().join("second");
    std::fs::create_dir(&first_cwd).expect("first workspace");
    std::fs::create_dir(&second_cwd).expect("second workspace");
    let fixture = fixture(cx);
    let first = cx
        .update_window(fixture.window.into(), |_, window, cx| {
            let first = fixture.view.update(cx, |view, cx| {
                view.backend.server =
                    bas::service::Target::Unix("/tmp/terminal-test.sock".into()).into();
                view.navigation.directories = vec![first_cwd.clone(), second_cwd.clone()];
                view.navigation.selected_directory = Some(first_cwd.clone());
                view.toggle_terminal(window, cx);
                view.terminal_panel()
                    .expect("first workspace provides its panel")
            });
            window.render_frame(cx);
            let group = window.find("workspace-header-0").bounds();
            let terminals = window.find("workspace-terminals").bounds();
            let dock = window.find("local-terminal-dock").bounds();
            assert!(
                terminals.origin.y > group.origin.y,
                "terminal controls belong below workspace heading"
            );
            assert!(
                terminals.right() <= px(SIDEBAR_WIDTH),
                "controls remain inside workspace navigation"
            );
            assert!(
                dock.origin.x >= px(SIDEBAR_WIDTH),
                "terminal content never takes over application sidebar"
            );
            first
        })
        .expect("workspace-owned terminal layout");
    cx.update_window(fixture.window.into(), |_, window, cx| {
        fixture.view.update(cx, |view, cx| {
            view.navigation.selected_directory = Some(second_cwd.clone());
            view.sync_terminal_panel(window, cx);
            let second = view.terminal_panel().expect("second workspace panel");
            assert_ne!(first.entity_id(), second.entity_id());
            assert!(
                view.render_workspace_terminals(Some(&first_cwd), cx)
                    .is_none()
            );
            assert!(
                view.render_workspace_terminals(Some(&second_cwd), cx)
                    .is_some()
            );
            view.navigation.selected_directory = Some(first_cwd.clone());
            assert_eq!(
                view.terminal_panel()
                    .expect("retained first panel")
                    .entity_id(),
                first.entity_id()
            );
            assert!(
                view.render_workspace_terminals(Some(&first_cwd), cx)
                    .is_some()
            );
            assert!(
                view.render_workspace_terminals(Some(&second_cwd), cx)
                    .is_none()
            );
        });
    })
    .expect("directory switch retains terminal owner");
    let shutdowns = cx.update(|cx| {
        fixture
            .view
            .update(cx, |view, cx| view.shutdown_terminals(cx))
    });
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
