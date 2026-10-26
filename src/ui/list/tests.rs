use super::Geometry;
use gpui_kit::{Bounds, point, px, size};

#[test]
fn blank_press_tracks_scrolled_rows_and_preserves_header_and_scrollbars() {
    let geometry = Geometry {
        rows: 5,
        row_height: px(40.),
        header_height: px(30.),
        inset_x: px(0.),
        bottom_inset: px(16.),
        scrollbar_width: px(16.),
    };
    let bounds = Bounds::new(point(px(20.), px(50.)), size(px(400.), px(300.)));
    let hit =
        |x, y, offset| geometry.is_blank(bounds, bounds.origin + point(px(x), px(y)), px(offset));
    assert!(
        !hit(100., 180., 0.),
        "a row retains its own selection behavior"
    );
    assert!(hit(100., 240., 0.), "space below the rows clears selection");
    assert!(
        hit(100., 180., -80.),
        "the live scroll offset moves the blank boundary"
    );
    assert!(!hit(100., 15., -80.), "column headings keep selection");
    assert!(
        !hit(390., 240., 0.),
        "the vertical scrollbar keeps selection"
    );
    assert!(
        !hit(100., 290., 0.),
        "the horizontal scrollbar keeps selection"
    );
    assert!(
        !hit(100., 310., 0.),
        "a press outside this list does not clear it"
    );
}
