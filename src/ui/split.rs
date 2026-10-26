//! A vertical split whose preferred height survives temporary space constraints.

use std::{cell::Cell, rc::Rc};

use gpui_kit::{component::ActiveTheme, prelude::*, *};

#[cfg(test)]
mod tests;

pub const DIVIDER_HEIGHT: f32 = 9.;
const KEY_STEP: f32 = 12.;
const KEY_CONTEXT: &str = "GdsVerticalSplit";

actions!(gds_split, [MoveUp, MoveDown]);
struct Bindings;
impl Global for Bindings {}

#[derive(Clone, Copy)]
pub struct Changed(pub f32);

pub struct State {
    focus: FocusHandle,
    preferred: Option<f32>,
    minimum_top: f32,
    minimum_bottom: f32,
    default_bottom: f32,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    dragging: bool,
}

impl EventEmitter<Changed> for State {}

impl State {
    pub fn new(
        preferred: Option<f32>,
        minimum_top: f32,
        minimum_bottom: f32,
        cx: &mut Context<Self>,
    ) -> Self {
        if cx.try_global::<Bindings>().is_none() {
            cx.bind_keys([
                KeyBinding::new("up", MoveUp, Some(KEY_CONTEXT)),
                KeyBinding::new("down", MoveDown, Some(KEY_CONTEXT)),
            ]);
            cx.set_global(Bindings);
        }
        Self {
            focus: cx.focus_handle(),
            preferred: preferred.filter(|height| height.is_finite() && *height > 0.),
            minimum_top,
            minimum_bottom,
            default_bottom: minimum_bottom,
            bounds: Rc::new(Cell::new(Bounds::default())),
            dragging: false,
        }
    }

    pub fn default_bottom(&mut self, height: f32) {
        self.default_bottom = height;
    }

    pub fn height(&self) -> f32 {
        constrained_height(
            self.preferred,
            self.bounds.get().size.height.into(),
            self.minimum_top,
            self.minimum_bottom,
            self.default_bottom,
        )
    }

    fn adjust(&mut self, height: f32, cx: &mut Context<Self>) {
        let available = f32::from(self.bounds.get().size.height);
        if available <= 0. {
            return;
        }
        self.preferred = Some(constrained_height(
            Some(height),
            available,
            self.minimum_top,
            self.minimum_bottom,
            self.default_bottom,
        ));
        cx.notify();
    }

    fn commit(&self, cx: &mut Context<Self>) {
        if let Some(height) = self.preferred {
            cx.emit(Changed(height));
        }
    }
}

pub fn constrained_height(
    preferred: Option<f32>,
    available: f32,
    top_min: f32,
    bottom_min: f32,
    default_bottom: f32,
) -> f32 {
    let usable = (available - DIVIDER_HEIGHT).max(0.);
    let top_min = top_min.min(usable / 2.);
    let maximum = (usable - bottom_min).max(top_min);
    preferred
        .filter(|value| value.is_finite())
        .unwrap_or(usable - default_bottom)
        .clamp(top_min, maximum)
}

#[derive(IntoElement)]
pub struct VerticalSplit {
    state: Entity<State>,
    top: AnyElement,
    bottom: AnyElement,
    label: SharedString,
}

impl VerticalSplit {
    pub fn new(
        state: Entity<State>,
        top: AnyElement,
        bottom: AnyElement,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            state,
            top,
            bottom,
            label: label.into(),
        }
    }
}

impl RenderOnce for VerticalSplit {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let height = state.height();
        let focus = state.focus.clone();
        let dragging = state.dragging;
        let bounds = state.bounds.clone();
        let measure_state = self.state.downgrade();
        let drag_state = self.state.downgrade();
        let measure = canvas(
            move |new_bounds, _, cx| {
                if bounds.replace(new_bounds) != new_bounds {
                    let _ = measure_state.update(cx, |_, cx| cx.notify());
                }
            },
            move |_, _, window, _| {
                let motion = drag_state.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let _ = motion.update(cx, |state, cx| {
                        if !state.dragging {
                            return;
                        }
                        if event.pressed_button != Some(MouseButton::Left) {
                            state.dragging = false;
                            state.commit(cx);
                            cx.notify();
                            return;
                        }
                        state.adjust(
                            f32::from(event.position.y - state.bounds.get().origin.y),
                            cx,
                        );
                    });
                });
                let release = drag_state.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                        return;
                    }
                    let _ = release.update(cx, |state, cx| {
                        if state.dragging {
                            state.dragging = false;
                            state.commit(cx);
                            cx.notify();
                        }
                    });
                });
            },
        )
        .absolute()
        .size_full();
        let press = self.state.clone();
        let keyboard_up = self.state.clone();
        let keyboard_down = self.state.clone();
        let line = cx.theme().border;
        let highlight = cx.theme().primary;
        let divider = div()
            .id("vertical-split-divider")
            .track_focus(&focus)
            .key_context(KEY_CONTEXT)
            .tab_index(0)
            .flex_shrink_0()
            .w_full()
            .h(px(DIVIDER_HEIGHT))
            .cursor(CursorStyle::ResizeUpDown)
            .flex()
            .items_center()
            .px_2()
            .bg(if dragging {
                highlight.opacity(0.16)
            } else {
                rgba(0).into()
            })
            .hover(|style| style.bg(highlight.opacity(0.12)))
            .focus(|style| style.bg(highlight.opacity(0.16)))
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(self.label.clone()).build(window, cx)
            })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                press.update(cx, |state, cx| {
                    state.dragging = true;
                    state.focus.focus(window, cx);
                    cx.notify();
                });
                cx.stop_propagation();
            })
            .on_action(move |_: &MoveUp, _, cx| {
                keyboard_up.update(cx, |state, cx| {
                    state.adjust(state.height() - KEY_STEP, cx);
                    state.commit(cx);
                });
                cx.stop_propagation();
            })
            .on_action(move |_: &MoveDown, _, cx| {
                keyboard_down.update(cx, |state, cx| {
                    state.adjust(state.height() + KEY_STEP, cx);
                    state.commit(cx);
                });
                cx.stop_propagation();
            })
            .child(
                div()
                    .w_full()
                    .h(px(1.))
                    .bg(if dragging { highlight } else { line }),
            );
        div()
            .id("vertical-split")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .h(px(height))
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.top),
            )
            .child(divider)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.bottom),
            )
            .child(measure)
    }
}
