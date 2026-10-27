use super::WindowControls;

#[test]
fn niri_session_hides_the_entire_control_group() {
    for desktop in ["niri", "Niri", "niri:GNOME", " custom : niri "] {
        assert!(WindowControls::session_hides_controls(Some(desktop), false));
    }
}

#[test]
fn declared_desktop_takes_precedence_over_inherited_niri_socket() {
    for desktop in ["GNOME", "KDE", "sway", "not-niri"] {
        assert!(!WindowControls::session_hides_controls(Some(desktop), true));
    }
}

#[test]
fn missing_desktop_uses_the_niri_socket_as_a_fallback() {
    assert!(WindowControls::session_hides_controls(None, true));
    assert!(WindowControls::session_hides_controls(Some(" "), true));
    assert!(!WindowControls::session_hides_controls(None, false));
}
