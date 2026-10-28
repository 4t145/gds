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
