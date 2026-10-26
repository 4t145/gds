use super::cycle_neighbour;

/// The page-cycle keys and the per-tab hints share one target function: the
/// strip order is the cycle order, the ends connect, one tab has no neighbour
/// and two tabs point both directions at the same row.
#[test]
fn a_page_cycle_lands_on_the_neighbour_the_tab_hint_names() {
    // Pinned tabs sort to the front, so the strip order is what cycles.
    let strip = [7, 3, 5, 9];

    assert_eq!(cycle_neighbour(&strip, 7, false), Some(3));
    assert_eq!(cycle_neighbour(&strip, 9, false), Some(7));
    assert_eq!(cycle_neighbour(&strip, 7, true), Some(9));
    assert_eq!(cycle_neighbour(&strip, 5, true), Some(3));

    assert_eq!(cycle_neighbour(&[4], 4, false), None);
    assert_eq!(cycle_neighbour(&[4], 4, true), None);

    assert_eq!(cycle_neighbour(&[1, 2], 1, false), Some(2));
    assert_eq!(cycle_neighbour(&[1, 2], 1, true), Some(2));
    assert_eq!(cycle_neighbour(&[1, 2], 2, false), Some(1));
}
