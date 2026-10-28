use super::Panel;
use gpui_kit::{AppContext, TestAppContext};
use std::task::{Context as TaskContext, Poll, Waker};

#[gpui_kit::test]
fn independent_empty_collections_do_not_spawn_when_targets_or_preferences_change(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        let first = cx.new(Panel::new);
        let second = cx.new(Panel::new);
        first.update(cx, |panel, cx| {
            panel.set_cwd(
                Some(std::env::current_dir().expect("working directory")),
                cx,
            );
            let mut preferences = gds::terminal::Preferences::default();
            preferences.login_shell = true;
            panel.set_preferences(preferences, cx);
            assert!(panel.entries.is_empty());
            assert!(panel.active.is_none());
        });
        assert!(second.read(cx).cwd.is_none());
        assert!(!second.read(cx).preferences.login_shell);
        first.update(cx, |panel, cx| panel.set_cwd(None, cx));
        assert!(first.read(cx).entries.is_empty());
        assert!(first.read(cx).cwd.is_none());
    });
}

#[gpui_kit::test]
fn empty_collection_shutdown_is_repeatable_and_does_not_seal_other_owners(cx: &mut TestAppContext) {
    let (mut first, mut repeated) = cx.update(|cx| {
        let panel = cx.new(Panel::new);
        let other = cx.new(Panel::new);
        let futures = panel.update(cx, |panel, cx| {
            let first = panel.shutdown(cx);
            assert!(panel.sealed);
            assert!(panel.entries.is_empty());
            (first, panel.shutdown(cx))
        });
        assert!(!other.read(cx).sealed);
        futures
    });
    let mut task = TaskContext::from_waker(Waker::noop());
    assert_eq!(first.as_mut().poll(&mut task), Poll::Ready(()));
    assert_eq!(repeated.as_mut().poll(&mut task), Poll::Ready(()));
}

#[gpui_kit::test]
async fn close_starting_terminal_skips_dialog_and_preserves_shutdown_barrier(
    cx: &mut TestAppContext,
) {
    use gpui_kit::{component::Root, px, size};
    cx.executor().allow_parking();
    let directory = tempfile::tempdir().expect("terminal directory");
    cx.update(|cx| {
        gpui_kit::init(cx);
        gds::i18n::install(gds::i18n::Language::English, cx).expect("test messages");
    });
    let mut panel = None;
    let window = cx.open_window(size(px(900.), px(500.)), |window, cx| {
        let view = cx.new(Panel::new);
        panel = Some(view.clone());
        Root::new(view, window, cx)
    });
    let panel = panel.expect("terminal panel");
    let shutdown = window
        .update(cx, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.preferences.shell = Some(directory.path().join("missing-test-shell"));
                panel.open(directory.path().to_path_buf(), window, cx);
                let id = panel.active.expect("new terminal");
                let terminal = panel.entries[0].terminal.clone();
                panel.request_close(id, window, cx);
                panel.request_close(id, window, cx);
                assert!(panel.entries[0].closing, "startup is cancelled immediately");
                // Collection closure must include the already closing entry.
                (panel.shutdown(cx), terminal)
            })
        })
        .expect("close starting terminal");
    let (shutdown, terminal) = shutdown;
    shutdown.await;
    cx.run_until_parked();
    cx.update(|cx| assert!(!terminal.read(cx).is_running(cx)));
}
