use super::{Command, SETTINGS_CONTEXT, WORKSPACE_CONTEXT, bindings};
use gpui_kit::{AsKeystroke, KeyContext, Keymap, Keystroke};

fn context(source: &'static str) -> Vec<KeyContext> {
    vec![KeyContext::parse(source).expect("bundled contexts are valid")]
}

fn keystroke(source: &str) -> Keystroke {
    Keystroke::parse(source).expect("bundled keystrokes are valid")
}

/// The refresh keycap follows the primary stroke while the compatibility one
/// keeps working, and neither leaks into the settings workspace.
#[test]
fn refresh_displays_ctrl_r_and_keeps_f5() {
    let keymap = Keymap::new(bindings());
    let action = Command::Refresh.action();

    let displayed = keymap
        .bindings_for_action(action.as_ref())
        .last()
        .expect("refresh is bound");
    let displayed = displayed.keystrokes()[0].as_keystroke();
    assert!(displayed.modifiers.control);
    assert_eq!(displayed.key, "r");

    for source in [Command::Refresh.default_stroke(), "f5"] {
        let (resolved, pending) =
            keymap.bindings_for_input(&[keystroke(source)], &context(WORKSPACE_CONTEXT));
        assert!(!pending, "{source} is not a prefix of a longer binding");
        assert_eq!(resolved.len(), 1, "{source} resolves one binding");
        assert!(
            resolved[0].action().partial_eq(action.as_ref()),
            "{source} runs refresh"
        );

        let (resolved, _) =
            keymap.bindings_for_input(&[keystroke(source)], &context(SETTINGS_CONTEXT));
        assert!(resolved.is_empty(), "{source} stays out of settings");
    }
}

/// `Alt+M` is the displayed menu key, `F10` keeps working, and both run the one
/// menu action in the file workspace only.
#[test]
fn menu_displays_alt_m_and_keeps_f10() {
    let keymap = Keymap::new(bindings());
    let action = Command::Menu.action();

    let displayed = keymap
        .bindings_for_action(action.as_ref())
        .last()
        .expect("the menu is bound");
    let displayed = displayed.keystrokes()[0].as_keystroke();
    assert!(displayed.modifiers.alt);
    assert_eq!(displayed.key, "m");

    for source in [Command::Menu.default_stroke(), "f10"] {
        let (resolved, pending) =
            keymap.bindings_for_input(&[keystroke(source)], &context(WORKSPACE_CONTEXT));
        assert!(!pending, "{source} is not a prefix of a longer binding");
        assert_eq!(resolved.len(), 1, "{source} resolves one binding");
        assert!(
            resolved[0].action().partial_eq(action.as_ref()),
            "{source} opens the menu"
        );

        let (resolved, _) =
            keymap.bindings_for_input(&[keystroke(source)], &context(SETTINGS_CONTEXT));
        assert!(resolved.is_empty(), "{source} stays out of settings");
    }
}
