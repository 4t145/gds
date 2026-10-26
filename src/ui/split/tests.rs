use super::constrained_height;

#[test]
fn shrinking_the_workspace_does_not_replace_the_preferred_height() {
    let preferred = Some(340.);
    assert_eq!(constrained_height(preferred, 260., 90., 100., 180.), 151.);
    assert_eq!(constrained_height(preferred, 700., 90., 100., 180.), 340.);
    let automatic = constrained_height(None, 700., 90., 100., 180.);
    assert_eq!(automatic, 511.);
}
