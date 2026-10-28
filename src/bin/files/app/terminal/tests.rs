use super::{CloseTarget, local_cwd};
use crate::app::{HOME_TAB_ID, OpenTab};
use std::path::{Path, PathBuf};

#[test]
fn cwd_never_uses_home_for_virtual_or_unavailable_locations() {
    for path in [
        "",
        ".",
        "smb://host/share",
        "trash://",
        "recent://",
        "~/project",
    ] {
        assert_eq!(local_cwd(Path::new(path), true), None, "{path}");
    }
    let local = std::env::current_dir().expect("test working directory");
    assert_eq!(local_cwd(&local, false), None);
    assert_eq!(local_cwd(&local, true), Some(local));
}

#[test]
fn home_and_volume_slots_remain_distinct_from_ordinary_tab_owners() {
    let home = OpenTab::fixed(HOME_TAB_ID, PathBuf::from("/home/user"));
    let ordinary = OpenTab::new(1, PathBuf::from("/home/user"), false);
    let volume = OpenTab::fixed(2, PathBuf::from("/mnt/volume"));
    let slots = [home, ordinary, volume];
    assert!(
        slots.iter().all(|slot| slot.terminal.is_none()),
        "creating a file slot never spawns a shell"
    );
    for slot in &slots {
        assert!(CloseTarget::Window.includes(slot.id));
        assert_eq!(CloseTarget::Tab(1).includes(slot.id), slot.id == 1);
    }
    assert!(
        !CloseTarget::Tab(1).includes(HOME_TAB_ID),
        "same cwd does not merge owners"
    );
}
