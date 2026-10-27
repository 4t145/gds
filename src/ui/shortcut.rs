//! Effective bindings and their shared visual and text presentation.
use super::*;

/// An action together with the key context whose binding applies to it.
#[derive(Clone)]
pub struct Shortcut {
    action: Rc<dyn Action>,
    context: Option<&'static str>,
    prefix: Option<PrefixTarget>,
}

/// A shortcut's entry action when it is reached through a keyboard prefix.
#[derive(Clone)]
pub struct PrefixTarget {
    entry: Rc<Shortcut>,
}

impl PrefixTarget {
    pub fn entry(&self) -> &Shortcut {
        &self.entry
    }
}
impl PartialEq for PrefixTarget {
    fn eq(&self, other: &Self) -> bool {
        self.entry.same_scope(&other.entry)
    }
}
impl Eq for PrefixTarget {}

impl Shortcut {
    pub fn new(action: Box<dyn Action>, context: Option<&'static str>) -> Self {
        Self {
            action: Rc::from(action),
            context,
            prefix: None,
        }
    }

    /// The same key names used by badges and held-modifier labels.
    pub fn label(&self, window: &Window) -> Option<String> {
        self.binding(window)
            .map(|binding| Self::binding_label(&binding))
    }

    pub fn binding_label(binding: &KeyBinding) -> String {
        binding
            .keystrokes()
            .iter()
            .map(|stroke| {
                stroke_labels(stroke.as_keystroke(), None)
                    .iter()
                    .map(AsRef::<str>::as_ref)
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn hint(&self, label: &str, window: &Window) -> String {
        self.label(window)
            .map_or_else(|| label.to_owned(), |keys| format!("{label} · {keys}"))
    }

    pub fn context(&self) -> Option<&'static str> {
        self.context
    }

    pub fn action(&self) -> &dyn Action {
        &*self.action
    }

    /// Mark this as a page key that only runs behind the prefix `entry` arms.
    pub fn via_prefix(mut self, entry: impl Into<Shortcut>) -> Self {
        self.prefix = Some(PrefixTarget {
            entry: Rc::new(entry.into()),
        });
        self
    }

    /// The prefix this key belongs behind, for a page key; `None` for an
    /// ordinary shortcut that runs from the plain layer itself.
    pub fn prefix(&self) -> Option<PrefixTarget> {
        self.prefix.clone()
    }

    /// Whether both shortcuts name the same action in the same scope, so a
    /// cached presentation does not have to be rebuilt.
    pub fn same_scope(&self, other: &Self) -> bool {
        self.context == other.context
            && self.prefix == other.prefix
            && self.action.partial_eq(&*other.action)
    }

    /// The highest-precedence binding currently in effect for this action.
    pub fn binding(&self, window: &Window) -> Option<KeyBinding> {
        let context = self
            .context
            .map(|value| KeyContext::parse(value).expect("bundled action contexts are valid"))
            .unwrap_or_default();
        window.highest_precedence_binding_for_action_in_context(&*self.action, context)
    }
}

impl From<Command> for Shortcut {
    fn from(command: Command) -> Self {
        Self::new(command.action(), command.context())
    }
}

/// The modifier group a press reveals hint labels for.
///
/// The two groups never mix. Ctrl+Alt (what AltGr reports) selects neither, so
/// such a press shows no labels instead of lighting the other group's keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeldModifier {
    Ctrl,
    Alt,
}

impl HeldModifier {
    /// The group the current modifier state asks for, if any.
    pub fn from_modifiers(modifiers: Modifiers) -> Option<Self> {
        if modifiers.platform {
            return None;
        }
        match (modifiers.control, modifiers.alt) {
            (true, false) => Some(Self::Ctrl),
            (false, true) => Some(Self::Alt),
            _ => None,
        }
    }

    /// Whether a stroke carries this group's modifier.
    pub fn matches(self, modifiers: Modifiers) -> bool {
        match self {
            Self::Ctrl => modifiers.control,
            Self::Alt => modifiers.alt,
        }
    }
}

#[derive(IntoElement)]
pub struct ShortcutBadge {
    source: BadgeSource,
}

/// Where a badge's keys come from: a control's own shortcut, resolved in its
/// declared scope, or a binding already resolved for the current context.
enum BadgeSource {
    Shortcut(Shortcut),
    Binding(KeyBinding),
}

impl ShortcutBadge {
    pub fn new(command: impl Into<Shortcut>) -> Self {
        Self::bound(command.into())
    }

    /// A badge for any action, resolved through its own key context.
    pub fn bound(shortcut: Shortcut) -> Self {
        Self {
            source: BadgeSource::Shortcut(shortcut),
        }
    }

    /// A badge for a binding that is already known to apply right now.
    pub fn binding(binding: KeyBinding) -> Self {
        Self {
            source: BadgeSource::Binding(binding),
        }
    }

    fn labels(&self, window: &Window) -> Vec<SharedString> {
        let binding = match &self.source {
            BadgeSource::Shortcut(shortcut) => shortcut.binding(window),
            BadgeSource::Binding(binding) => Some(binding.clone()),
        };
        key_labels(binding.as_ref())
    }

    /// The whole combination as one compact label, such as `R` or `Shift+Tab`,
    /// with the modifier the user is already holding left out because the press
    /// itself shows it.
    ///
    /// It is spelled from [`key_labels`], the same names the keycaps use, so the
    /// two presentations cannot drift apart.
    pub(crate) fn path_label(bindings: &[KeyBinding], held: HeldModifier) -> Option<SharedString> {
        let labels = bindings
            .iter()
            .flat_map(|binding| key_labels_omitting(Some(binding), Some(held)))
            .collect::<Vec<_>>();
        if labels.is_empty() {
            return None;
        }
        let parts = labels.iter().map(AsRef::<str>::as_ref).collect::<Vec<_>>();
        Some(parts.join("+").into())
    }
}

fn key_labels(binding: Option<&KeyBinding>) -> Vec<SharedString> {
    key_labels_omitting(binding, None)
}

/// The key names of a binding's first stroke, minus the prefix of `held`.
fn key_labels_omitting(
    binding: Option<&KeyBinding>,
    held: Option<HeldModifier>,
) -> Vec<SharedString> {
    binding
        .and_then(|binding| binding.keystrokes().first())
        .map(|stroke| stroke_labels(stroke.as_keystroke(), held))
        .unwrap_or_default()
}

fn stroke_labels(stroke: &Keystroke, held: Option<HeldModifier>) -> Vec<SharedString> {
    let mut labels = Vec::new();
    if stroke.modifiers.control && held != Some(HeldModifier::Ctrl) {
        labels.push("Ctrl".into());
    }
    if stroke.modifiers.alt && held != Some(HeldModifier::Alt) {
        labels.push("Alt".into());
    }
    if stroke.modifiers.shift {
        labels.push("Shift".into());
    }
    if stroke.modifiers.platform {
        labels.push("Super".into());
    }
    labels.push(match stroke.key.as_str() {
        "left" => "←".into(),
        "right" => "→".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "enter" => "Enter".into(),
        "escape" => "Esc".into(),
        "tab" => "Tab".into(),
        key => key.to_uppercase().into(),
    });
    labels
}

impl RenderOnce for ShortcutBadge {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let keys = self.labels(window).into_iter().map(Keycap::new);
        el! { .div { flex; items_center; gap: KEY_GAP.px } [..keys] }
    }
}

#[derive(IntoElement)]
struct Keycap {
    label: SharedString,
}

impl Keycap {
    fn new(label: SharedString) -> Self {
        Self { label }
    }
}

impl RenderOnce for Keycap {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let label = self.label;
        el! {
            .div {
                flex; items_center; justify_center;
                h: KEY_HEIGHT.px; min_w: KEY_HEIGHT.px; px: KEY_PADDING.px;
                rounded: KEY_GAP.px; border_1;
                border_color: $cx.theme().border;
                bg: $cx.theme().secondary; text_color: $cx.theme().muted_foreground;
                text_size: $(typography::scaled(KEY_TEXT_SIZE, cx));
            } [label]
        }
    }
}

#[derive(Clone)]
pub(super) struct Hint {
    label: SharedString,
    shortcut: Option<Shortcut>,
}

impl Hint {
    pub(super) fn command(label: SharedString, command: Option<Shortcut>) -> Self {
        Self {
            label,
            shortcut: command,
        }
    }

    pub(super) fn shortcut(label: SharedString, shortcut: Shortcut) -> Self {
        Self {
            label,
            shortcut: Some(shortcut),
        }
    }

    pub(super) fn builder(self) -> impl Fn(&mut Window, &mut App) -> AnyView {
        move |window, cx| {
            let hint = self.clone();
            super::tooltip::element(
                "command-tooltip",
                hint.label.clone(),
                move |_, _| hint.content().into_any_element(),
                window,
                cx,
            )
        }
    }

    fn content(&self) -> Div {
        let badge = self.shortcut.clone().map(ShortcutBadge::bound);
        el! {
            .div { flex; items_center; gap: HINT_GAP.px } [$(self.label.clone()), ..badge]
        }
    }
}

/// A tooltip holding a control's short label and the key that runs it.
///
/// The badge reads the highest-precedence binding like every other presentation
/// of a command, so a control whose `Command` it names cannot show another key
/// than the one a press runs.
pub fn command_tooltip(
    label: impl Into<SharedString>,
    command: impl Into<Shortcut>,
) -> impl Fn(&mut Window, &mut App) -> AnyView {
    Hint::command(label.into(), Some(command.into())).builder()
}

/// A tooltip holding a control's short label and an already-resolved binding.
///
/// It serves controls whose key is not one of the workspace [`Command`]s, such
/// as the picker's own decisions, while keeping the same presentation as every
/// other keyed control.
pub fn shortcut_tooltip(
    label: impl Into<SharedString>,
    shortcut: Shortcut,
) -> impl Fn(&mut Window, &mut App) -> AnyView {
    Hint::shortcut(label.into(), shortcut).builder()
}
