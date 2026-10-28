use super::{State, Status, Terminal};
use crate::terminal::Preferences;
use gpui_kit::{AppContext, TestAppContext};
use std::{
    path::PathBuf,
    task::{Context as TaskContext, Poll, Waker},
};

#[gpui_kit::test]
fn single_terminal_shutdown_waits_for_its_inflight_startup_and_is_idempotent(
    cx: &mut TestAppContext,
) {
    let (spawn_done, starting) = async_channel::bounded(1);
    let (terminal, mut first, mut second) = cx.update(|cx| {
        let terminal = cx.new(|cx| Terminal {
            cwd: PathBuf::from("/"),
            preferences: Preferences::default(),
            focus: cx.focus_handle(),
            state: State::Starting(starting),
            closing: false,
            shutdown_done: None,
            observed: None,
        });
        let (first, second) = terminal.update(cx, |terminal, cx| {
            assert_eq!(terminal.status(cx), Status::Starting);
            let first = terminal.shutdown(cx);
            assert_eq!(terminal.status(cx), Status::Stopping);
            let second = terminal.shutdown(cx);
            (first, second)
        });
        (terminal, first, second)
    });
    let mut task = TaskContext::from_waker(Waker::noop());
    cx.run_until_parked();
    assert_eq!(first.as_mut().poll(&mut task), Poll::Pending);
    assert_eq!(second.as_mut().poll(&mut task), Poll::Pending);
    // A cancelled/failed launch produces no Session, but still has to complete.
    drop(spawn_done);
    cx.run_until_parked();
    assert_eq!(first.as_mut().poll(&mut task), Poll::Ready(()));
    assert_eq!(second.as_mut().poll(&mut task), Poll::Ready(()));
    cx.update(|cx| assert_eq!(terminal.read(cx).status(cx), Status::Closed));
}
