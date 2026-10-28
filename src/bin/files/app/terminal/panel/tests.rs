use super::Panel;
use gpui_kit::{AppContext, TestAppContext, component::Root, px, size};
use std::task::{Context as TaskContext, Poll, Waker};

#[gpui_kit::test]
async fn revealing_an_empty_local_panel_starts_once_and_focuses_it(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let directory = tempfile::tempdir().expect("terminal test directory");
    cx.update(|cx| {
        gpui_kit::init(cx);
        gds::i18n::install(gds::i18n::Language::English, cx).expect("terminal translations");
    });
    let panel = cx.update(|cx| cx.new(Panel::new));
    let window = cx.open_window(size(px(900.), px(500.)), |window, cx| {
        Root::new(panel.clone(), window, cx)
    });
    let shutdown = window
        .update(cx, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.reveal(window, cx);
                assert!(
                    panel.entries.is_empty(),
                    "unknown cwd never falls back to Home"
                );
                panel.set_cwd(Some(directory.path().to_path_buf()), cx);
                panel.set_preferences(
                    gds::terminal::Preferences {
                        shell: Some(directory.path().join("missing-test-shell")),
                        ..Default::default()
                    },
                    cx,
                );
                panel.reveal(window, cx);
                let terminal = panel.entries[0].terminal.clone();
                assert!(panel.contains_focus(window, cx));
                panel.reveal(window, cx);
                assert_eq!(panel.entries.len(), 1);
                assert_eq!(panel.entries[0].terminal, terminal);
                panel.set_cwd(None, cx);
                panel.reveal(window, cx);
                assert_eq!(
                    panel.entries.len(),
                    1,
                    "unavailable target preserves existing instance"
                );
                panel.shutdown(cx)
            })
        })
        .expect("reveal local panel");
    shutdown.await;
}

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
