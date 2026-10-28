//! A single terminal, including asynchronous process startup and cleanup.
//! Collections, tab selection, pane layout and host navigation are application state.
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

use async_channel::Receiver;
use gpui_kit::{prelude::*, *};

use super::{
    Event, INPUT_CONTEXT, Preferences,
    session::{Session, SpawnOptions},
    view::View,
};
use crate::{i18n, theme};

/// Await before releasing a host window. Cleanup itself never blocks the UI thread.
pub type Shutdown = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Starting,
    Running,
    Stopping,
    Closed,
    Exited { code: u32, signal: Option<String> },
    Failed(String),
}
impl Status {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Stopping)
    }
}

enum State {
    Starting(Receiver<Session>),
    Ready {
        view: Entity<View>,
        _updates: Subscription,
    },
    Failed(String),
    Stopping,
    Closed,
}

/// One process and its terminal surface. No tabs, management controls or key map.
pub struct Terminal {
    cwd: PathBuf,
    preferences: Preferences,
    focus: FocusHandle,
    state: State,
    closing: bool,
    shutdown_done: Option<Receiver<()>>,
    observed: Option<(Status, Option<SharedString>)>,
}

impl Terminal {
    pub fn new(
        cwd: PathBuf,
        preferences: Preferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let preferences = preferences.normalized();
        let options = SpawnOptions {
            cwd: cwd.clone(),
            shell: preferences.shell.clone(),
            args: preferences.shell_args.clone(),
            login: preferences.login_shell,
            columns: 80,
            rows: 24,
            scrollback: preferences.scrollback_lines,
            palette: Some(super::view::palette::Palette::new(cx).query_colors()),
        };
        let (spawned, completion) = async_channel::bounded(1);
        let spawn = cx.background_executor().spawn(async move {
            let result = Session::spawn(options);
            if let Ok(session) = &result {
                match spawned.try_send(session.clone()) {
                    Ok(()) | Err(async_channel::TrySendError::Closed(_)) => {}
                    Err(async_channel::TrySendError::Full(_)) => unreachable!("one startup result"),
                }
            }
            drop(spawned);
            result
        });
        // Always take ownership of an in-flight spawn's result. If the entity or
        // window is gone, dropping the returned Session requests process cleanup.
        cx.spawn_in(window, async move |terminal, cx| {
            let result = spawn.await;
            if let Err(error) = terminal.update_in(cx, move |this, window, cx| {
                if this.closing {
                    if let Ok(session) = result {
                        session.terminate();
                    }
                    return;
                }
                this.state = match result {
                    Ok(session) => {
                        let view =
                            cx.new(|cx| View::new(session, this.preferences.clone(), window, cx));
                        let updates = cx.observe(&view, |this, _, cx| this.changed(cx));
                        if this.focus.is_focused(window) {
                            let focus = view.read(cx).focus_handle();
                            focus.focus(window, cx);
                        }
                        State::Ready {
                            view,
                            _updates: updates,
                        }
                    }
                    Err(error) => State::Failed(error.to_string()),
                };
                this.changed(cx);
            }) {
                eprintln!("terminal startup owner closed: {error}");
            }
        })
        .detach();
        Self {
            cwd,
            preferences,
            focus: cx.focus_handle(),
            state: State::Starting(completion),
            closing: false,
            shutdown_done: None,
            observed: None,
        }
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn status(&self, cx: &App) -> Status {
        match &self.state {
            State::Starting(_) => {
                if self.closing {
                    Status::Stopping
                } else {
                    Status::Starting
                }
            }
            State::Ready { view, .. } => match view.read(cx).session.status() {
                super::session::Status::Running => Status::Running,
                super::session::Status::Stopping => Status::Stopping,
                super::session::Status::Exited { code, signal } => Status::Exited { code, signal },
                super::session::Status::Failed(error) => Status::Failed(error),
            },
            State::Failed(error) => Status::Failed(error.clone()),
            State::Stopping => Status::Stopping,
            State::Closed => Status::Closed,
        }
    }

    pub fn is_running(&self, cx: &App) -> bool {
        self.status(cx).is_running()
    }

    /// Submit an explicit folder switch to this existing instance, without
    /// restarting it or interrupting a foreground program. Call only at a prompt.
    ///
    /// # Errors
    /// Fails for unavailable paths, a non-ready/busy session or unsupported shell.
    pub fn change_directory(
        &self,
        cwd: PathBuf,
        cx: &App,
    ) -> impl Future<Output = anyhow::Result<()>> + Send + 'static + use<> {
        let session = match &self.state {
            State::Ready { view, .. } if !self.closing => Some(view.read(cx).session.clone()),
            _ => None,
        };
        async move {
            let session = session.ok_or(super::session::Error::ShellBusy)?;
            session.change_directory(&cwd).await?;
            Ok(())
        }
    }

    /// The process-supplied title. Hosts choose their own fallback and tab caption.
    pub fn title(&self, cx: &App) -> Option<SharedString> {
        match &self.state {
            State::Ready { view, .. } => view
                .read(cx)
                .session
                .title()
                .filter(|title| !title.is_empty())
                .map(Into::into),
            _ => None,
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle(cx).focus(window, cx);
    }

    pub fn contains_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
            || self.focus_handle(cx).contains_focused(window, cx)
    }

    /// Shell/argv changes only affect future instances. Existing processes are never restarted.
    pub fn set_preferences(&mut self, preferences: Preferences, cx: &mut Context<Self>) {
        let preferences = preferences.normalized();
        if self.preferences == preferences {
            return;
        }
        self.preferences = preferences;
        if let State::Ready { view, .. } = &self.state {
            view.update(cx, |view, cx| {
                view.set_preferences(self.preferences.clone(), cx)
            });
        }
        cx.notify();
    }

    /// End this instance, including a still-running startup, and wait for PTY cleanup.
    /// Calling again returns a waiter for the same operation, not an early success.
    pub fn shutdown(&mut self, cx: &mut Context<Self>) -> Shutdown {
        if let Some(done) = &self.shutdown_done {
            return wait_for_cleanup(done.clone());
        }
        self.closing = true;
        let cleanup: Shutdown = match &self.state {
            State::Ready { view, .. } => {
                let session = view.read(cx).session.clone();
                session.terminate();
                Box::pin(async move {
                    session.wait_stopped().await;
                })
            }
            State::Starting(starting) => {
                let starting = starting.clone();
                self.state = State::Stopping;
                Box::pin(async move {
                    if let Ok(session) = starting.recv().await {
                        session.terminate();
                        session.wait_stopped().await;
                    }
                })
            }
            State::Failed(_) | State::Stopping | State::Closed => Box::pin(async {}),
        };
        let (finished, done) = async_channel::bounded::<()>(1);
        cx.background_executor()
            .spawn(async move {
                cleanup.await;
                drop(finished);
            })
            .detach();
        self.shutdown_done = Some(done.clone());
        let completed = done.clone();
        cx.spawn(async move |terminal, cx| {
            let _ = completed.recv().await;
            let _ = terminal.update(cx, |this, cx| {
                if matches!(this.state, State::Stopping) {
                    this.state = State::Closed;
                }
                this.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
        wait_for_cleanup(done)
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        let state = (self.status(cx), self.title(cx));
        if self.observed.as_ref() != Some(&state) {
            self.observed = Some(state);
            cx.emit(Event::Changed);
        }
        cx.notify();
    }
}

fn wait_for_cleanup(done: Receiver<()>) -> Shutdown {
    Box::pin(async move {
        match done.recv().await {
            Err(_) => {}
            Ok(()) => unreachable!("completion is signalled by closing the channel"),
        }
    })
}

impl Focusable for Terminal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match &self.state {
            State::Ready { view, .. } => view.read(cx).focus_handle(),
            _ => self.focus.clone(),
        }
    }
}
impl EventEmitter<Event> for Terminal {}
impl Render for Terminal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.state {
            State::Ready { view, .. } => view.clone().into_any_element(),
            state => {
                let (label, detail) = match state {
                    State::Starting(_) => ("gds-terminal-starting", None),
                    State::Failed(error) => ("gds-terminal-start-failed", Some(error.clone())),
                    State::Closed => ("gds-terminal-exited", None),
                    _ => ("gds-terminal-stopping", None),
                };
                div()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .key_context(INPUT_CONTEXT)
                    .track_focus(&self.focus)
                    .bg(theme::content_background(cx))
                    .text_color(theme::Colors::current(cx).foreground)
                    .child(i18n::text(label, cx))
                    .when_some(detail, |this, detail| {
                        this.child(
                            div()
                                .text_color(theme::Colors::current(cx).error)
                                .child(detail),
                        )
                    })
                    .into_any_element()
            }
        }
    }
}

#[cfg(all(test, feature = "gui-tests"))]
mod tests;
