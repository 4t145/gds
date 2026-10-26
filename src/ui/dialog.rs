//! Shared dialog decisions: one footer with icons, labels and the single
//! Alt+A / Alt+C bindings that reach the dialog's own Confirm / Cancel actions.

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::dialog::{Cancel, Confirm};
use gpui_kit::{prelude::*, *};

use super::{Hint, Shortcut, TOOLTIP_DELAY, keytips};

/// Key context `gpui-base` puts on the modal host that owns the Escape and
/// Enter bindings; the dialog bindings here attach to the same host.
pub const DIALOG_CONTEXT: &str = "Dialog";

const ACTION_GAP: f32 = 8.0;

/// The two decisions every dialog offers, each with one shared binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogCommand {
    Confirm,
    Cancel,
}

impl DialogCommand {
    pub const ALL: [Self; 2] = [Self::Confirm, Self::Cancel];

    pub fn label_id(self) -> &'static str {
        match self {
            Self::Confirm => "dialog-confirm",
            Self::Cancel => "dialog-cancel",
        }
    }

    pub fn default_stroke(self) -> &'static str {
        match self {
            Self::Confirm => "alt-a",
            Self::Cancel => "alt-c",
        }
    }

    /// The existing `gpui-base` actions, so the keyboard path and the click
    /// path reach the same dialog callbacks.
    pub fn action(self) -> Box<dyn Action> {
        match self {
            Self::Confirm => Box::new(Confirm { secondary: false }),
            Self::Cancel => Box::new(Cancel),
        }
    }

    pub fn context(self) -> Option<&'static str> {
        Some(DIALOG_CONTEXT)
    }

    pub fn shortcut(self) -> Shortcut {
        Shortcut::new(self.action(), self.context())
    }

    fn button_id(self) -> &'static str {
        match self {
            Self::Confirm => "dialog-confirm",
            Self::Cancel => "dialog-cancel",
        }
    }

    /// Distinct from [`Self::button_id`], whose element state is the button's
    /// own focus handle.
    fn anchor_key(self) -> &'static str {
        match self {
            Self::Confirm => "dialog-confirm-anchor",
            Self::Cancel => "dialog-cancel-anchor",
        }
    }

    /// Distinct from [`Self::anchor_key`], whose element state is the focus
    /// handle; one element path cannot hold both states.
    fn tooltip_id(self) -> &'static str {
        match self {
            Self::Confirm => "dialog-confirm-hint",
            Self::Cancel => "dialog-cancel-hint",
        }
    }

    fn binding(self) -> KeyBinding {
        let context = KeyBindingContextPredicate::parse(DIALOG_CONTEXT)
            .expect("bundled action contexts are valid")
            .into();
        KeyBinding::load(
            self.default_stroke(),
            self.action(),
            Some(context),
            false,
            None,
            &DummyKeyboardMapper,
        )
        .expect("bundled dialog shortcuts are valid")
    }
}

/// Bind the dialog decisions once at startup instead of per dialog.
pub fn register(cx: &mut App) {
    cx.bind_keys(DialogCommand::ALL.into_iter().map(DialogCommand::binding));
}

/// A dialog control's own focus node, so activating the control dispatches its
/// action on the dialog it belongs to rather than on whatever holds focus.
///
/// `Window::dispatch_action` routes along the focused element; dispatching from
/// a node inside the control walks the control's own ancestors, which end at
/// the dialog that owns it, stacked dialogs included.
#[derive(Clone)]
struct DispatchAnchor {
    handle: FocusHandle,
}

impl DispatchAnchor {
    fn new(key: &'static str, window: &mut Window, cx: &mut App) -> Self {
        let handle = window
            .use_keyed_state(key, cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        Self { handle }
    }

    /// Zero-size, out of flow and never hovered: it only gives the handle a
    /// place in the dispatch tree, not in the tab order.
    fn element(&self) -> Div {
        div().absolute().size_0().track_focus(&self.handle)
    }

    fn dispatch(&self, action: &dyn Action, window: &mut Window, cx: &mut App) {
        if self.handle.is_focused(window) || self.handle.contains(&self.handle, window) {
            self.handle.dispatch_action(action, window, cx);
        } else {
            window.dispatch_action(action.boxed_clone(), cx);
        }
    }
}

/// One dialog decision as rendered in a footer.
struct DecisionButton {
    command: DialogCommand,
    label: SharedString,
    icon: IconName,
    variant: ButtonVariant,
}

impl DecisionButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let anchor = DispatchAnchor::new(self.command.anchor_key(), window, cx);
        let action = self.command.action();
        let label = self.label;
        // Only the open dialog's own decisions carry a binding, so the label
        // layer shows Alt+A / Alt+C here and no workspace key behind the modal.
        // The slot sits after the label inside the button, so a hint never
        // covers the button's text or the count in a destructive confirmation,
        // and its stable width keeps the footer from reflowing.
        let hint = keytips::suffix_anchor(
            window,
            cx,
            self.command.button_id().into(),
            self.command.shortcut(),
        );
        let button = Button::new(self.command.button_id())
            .with_variant(self.variant)
            .icon(Icon::new(self.icon))
            .label(label.clone())
            .child(anchor.element())
            .child(hint)
            .on_click(move |_, window, cx| anchor.dispatch(action.as_ref(), window, cx));
        let tooltip = Hint::shortcut(label, self.command.shortcut()).builder();
        div()
            .id(self.command.tooltip_id())
            .flex_shrink_0()
            .tooltip_show_delay(TOOLTIP_DELAY)
            .tooltip(tooltip)
            .child(button)
    }
}

/// The footer every dialog shares: cancel on the left of the primary decision,
/// both with icon and label, and tooltips that expose the Alt+A / Alt+C keys.
///
/// It renders and dispatches only. The dialog keeps its `on_ok`, `on_cancel`
/// and `on_close` callbacks, so validation, drag cancellation and closing stay
/// on the existing path.
#[derive(IntoElement)]
pub struct DialogActions {
    primary_label: SharedString,
    primary_icon: IconName,
    cancel_label: SharedString,
    danger: bool,
}

impl DialogActions {
    pub fn new(
        primary_label: impl Into<SharedString>,
        primary_icon: IconName,
        cancel_label: impl Into<SharedString>,
    ) -> Self {
        Self {
            primary_label: primary_label.into(),
            primary_icon,
            cancel_label: cancel_label.into(),
            danger: false,
        }
    }

    /// Style the primary decision as destructive; `Primary` is the default.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

impl RenderOnce for DialogActions {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let primary = DecisionButton {
            command: DialogCommand::Confirm,
            label: self.primary_label,
            icon: self.primary_icon,
            variant: if self.danger {
                ButtonVariant::Danger
            } else {
                ButtonVariant::Primary
            },
        };
        let cancel = DecisionButton {
            command: DialogCommand::Cancel,
            label: self.cancel_label,
            icon: IconName::X,
            variant: ButtonVariant::Default,
        };
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_end()
            .gap(px(ACTION_GAP))
            .child(cancel.render(window, cx))
            .child(primary.render(window, cx))
    }
}

#[cfg(test)]
mod tests;
