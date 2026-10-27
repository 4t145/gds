use super::{Availability, DeviceLocation, OpenTab, Snapshot, VolumeId, storage};
use std::{path::Path, sync::Arc};
use storage::Volume;

fn medium(identity: &str, root: &str) -> Snapshot {
    Snapshot {
        volumes: vec![Volume {
            id: VolumeId {
                stable: Some(identity.into()),
                attachment: identity.into(),
            },
            object: "backend-object".into(),
            drive: None,
            name: "Data".into(),
            size: 0,
            label: "Data".into(),
            filesystem: "ext4".into(),
            read_only: false,
            locked: false,
            mountable: true,
            system_mount: false,
            mount_points: vec![root.into()],
            visible: true,
        }],
        ..Snapshot::default()
    }
}

#[test]
fn disconnect_retains_relative_location_and_requires_explicit_reconnection() {
    let original = medium("original", "/media/data");
    let mut location = DeviceLocation::capture(&original, Path::new("/media/data/projects/rust"))
        .expect("mounted volume");
    location.update(&Snapshot::default());
    assert_eq!(location.availability, Availability::Disconnected);
    assert_eq!(location.relative, Path::new("projects/rust"));
    location.update(&original);
    assert_eq!(
        location.availability,
        Availability::Disconnected,
        "discovery alone never resumes file operations"
    );
    let replacement = medium("another-disk", "/media/data");
    assert!(!location.is_current(&replacement));
    assert!(replacement.reconnect(&location.volume).is_none());
}

#[test]
fn external_unmount_keeps_the_tab_but_hides_its_file_rows() {
    let original = medium("volume", "/media/data");
    let mut location = DeviceLocation::capture(&original, Path::new("/media/data/folder"))
        .expect("mounted volume");
    let mut unmounted = original.clone();
    unmounted.volumes[0].mount_points.clear();
    location.update(&unmounted);
    let mut slot = OpenTab::new(7, "/media/data/folder".into(), false);
    slot.device = Some(location);
    slot.visible = Arc::from([0]);
    slot.refresh_visible();
    assert_eq!(slot.path(), Path::new("/media/data/folder"));
    assert!(!slot.files_available());
    assert!(slot.visible.is_empty());
    assert!(slot.table_rows.is_empty());
}

#[test]
fn system_volume_entries_leave_ordinary_local_browsing_independent_of_discovery() {
    let mut snapshot = medium("system", "/");
    snapshot.volumes[0].system_mount = true;
    assert!(snapshot.at_path(Path::new("/home/user")).is_some());
    assert!(DeviceLocation::capture(&snapshot, Path::new("/home/user")).is_none());
}

#[test]
fn default_browsing_root_is_explicit_and_does_not_follow_discovery_order() {
    use super::pages::default_root;
    let mut snapshot = medium("system", "/nix/store");
    let volume = &mut snapshot.volumes[0];
    volume.mount_points.push("/".into());
    assert_eq!(default_root(volume, None).as_deref(), Some(Path::new("/")));
    assert_eq!(
        default_root(volume, Some(Path::new("/nix/store"))).as_deref(),
        Some(Path::new("/nix/store"))
    );
    volume
        .mount_points
        .retain(|root| root != Path::new("/nix/store"));
    assert_eq!(
        default_root(volume, Some(Path::new("/nix/store"))).as_deref(),
        Some(Path::new("/nix/store")),
        "a lost explicit root must not silently fall back to a different directory"
    );
}

#[test]
fn fixed_volume_pages_preserve_state_and_distinguish_media_reusing_a_mount_path() {
    let first = medium("one", "/media/one");
    let second = medium("two", "/media/one");
    let mut pages = super::pages::Pages::default();
    let mut next_id = 8;
    let one = pages.ensure(
        &first.volumes[0].id,
        Some(Path::new("/media/one")),
        &mut next_id,
    );
    let two = pages.ensure(
        &second.volumes[0].id,
        Some(Path::new("/media/one")),
        &mut next_id,
    );
    pages.entries[0].slot.filter = "preserved filter".into();
    assert_ne!(one, two);
    assert_eq!(pages.ensure(&first.volumes[0].id, None, &mut next_id), one);
    assert_eq!(pages.entries.len(), 2);
    assert_eq!(
        pages.active(one).expect("fixed page").slot.filter,
        "preserved filter"
    );
    assert_eq!(
        pages.active(one).expect("fixed page").slot.path(),
        Path::new("/media/one")
    );
    assert_eq!(next_id, 10);
}
