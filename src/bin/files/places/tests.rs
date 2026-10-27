use std::path::PathBuf;

use crate::settings::columns::{Column, Layout};

use super::*;

fn folder(path: &str) -> PathBuf {
    PathBuf::from(path)
}

#[test]
fn folder_overrides_are_isolated_and_clearing_restores_the_global_layout() {
    let global = Layout::default();
    let first = Layout::new([Column::Name, Column::Size]);
    let second = Layout::new([Column::Name, Column::Owner, Column::Accessed]);
    let first_path = folder("/tmp/one");
    let second_path = folder("/tmp/two");
    let uncovered = folder("/tmp/three");
    let mut places = Places::default();

    places.set_columns(&first_path, first.clone());
    places.set_columns(&second_path, second.clone());
    assert_eq!(places.columns_for(&first_path), Some(&first));
    assert_eq!(places.effective_columns(&first_path, &global), &first);
    assert_eq!(places.effective_columns(&second_path, &global), &second);
    assert_eq!(places.effective_columns(&uncovered, &global), &global);

    let replaced = Layout::new([Column::Name, Column::Permissions]);
    places.set_columns(&first_path, replaced.clone());
    assert_eq!(places.columns_for(&first_path), Some(&replaced));
    assert_eq!(places.columns_for(&second_path), Some(&second));
    assert_eq!(places.column_overrides.len(), 2);

    assert!(places.clear_columns(&first_path));
    assert_eq!(places.effective_columns(&first_path, &global), &global);
    assert_eq!(places.effective_columns(&second_path, &global), &second);
    assert!(!places.clear_columns(&first_path));
}

#[test]
fn overrides_apply_to_exact_paths_without_parent_or_child_inheritance() {
    let global = Layout::default();
    let child_layout = Layout::new([Column::Name, Column::Group]);
    let parent = folder("/tmp/project");
    let child = folder("/tmp/project/src");
    let mut places = Places::default();

    places.set_columns(&child, child_layout.clone());
    assert_eq!(places.effective_columns(&parent, &global), &global);
    assert_eq!(places.effective_columns(&child, &global), &child_layout);
    assert_eq!(
        places.effective_columns(&folder("/tmp/project/src2"), &global),
        &global
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_folder_paths_roundtrip_through_json() {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let root = tempfile::tempdir().expect("create places fixture");
    let file = root.path().join("places.json");
    let path = PathBuf::from(OsString::from_vec(b"/tmp/caf\xe9".to_vec()));
    let layout = Layout::new([Column::Name, Column::Owner, Column::Permissions]);
    let mut places = Places::default();
    places.favorites.push(OsString::from("/tmp/favorite"));
    places.set_columns(&path, layout.clone());

    save_to(&file, &places).expect("save places");
    let loaded = load_from(&file).expect("load places");

    assert_eq!(loaded.favorites, places.favorites);
    assert_eq!(loaded.columns_for(&path), Some(&layout));
    assert_eq!(
        loaded.column_overrides[0].path.as_os_str().as_bytes(),
        path.as_os_str().as_bytes()
    );
}

#[test]
fn legacy_places_without_column_overrides_load_and_keep_their_data() {
    let root = tempfile::tempdir().expect("create places fixture");
    let file = root.path().join("places.json");
    let legacy = serde_json::json!({
        "favorites": [{"Unix": "/tmp/favorite".as_bytes()}],
        "recent": [{"Unix": "/tmp/recent".as_bytes()}],
        "pinned": [{"Unix": "/tmp/pinned".as_bytes()}],
    });
    fs::write(
        &file,
        serde_json::to_vec(&legacy).expect("encode legacy places"),
    )
    .expect("write legacy places");

    let mut places = load_from(&file).expect("load legacy places");
    assert!(places.column_overrides.is_empty());
    assert_eq!(places.favorites, vec![OsString::from("/tmp/favorite")]);
    assert_eq!(places.recent, vec![OsString::from("/tmp/recent")]);
    assert_eq!(places.pinned, vec![OsString::from("/tmp/pinned")]);

    places.set_columns(
        &folder("/tmp/favorite"),
        Layout::new([Column::Name, Column::Size]),
    );
    save_to(&file, &places).expect("rewrite places");
    let reloaded = load_from(&file).expect("reload places");
    assert_eq!(reloaded.favorites, places.favorites);
    assert_eq!(reloaded.recent, places.recent);
    assert_eq!(reloaded.pinned, places.pinned);
    assert!(reloaded.columns_for(&folder("/tmp/favorite")).is_some());
}

#[test]
fn malformed_places_are_reported_and_not_overwritten() {
    let root = tempfile::tempdir().expect("create places fixture");
    let file = root.path().join("places.json");
    let invalid = br#"{"favorites": [}"#;
    fs::write(&file, invalid).expect("write malformed places");

    assert!(matches!(load_from(&file), Err(Error::Data(_))));
    assert_eq!(
        fs::read(&file).expect("read unchanged places"),
        invalid.to_vec()
    );
}

#[cfg(unix)]
#[test]
fn read_only_places_are_not_replaced() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().expect("create places fixture");
    let file = root.path().join("places.json");
    let contents = br#"{"favorites":[],"recent":[],"pinned":[]}"#;
    fs::write(&file, contents).expect("write places");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).expect("make places readonly");

    assert!(matches!(
        save_to(&file, &Places::default()),
        Err(Error::ReadOnly(_))
    ));
    assert_eq!(
        fs::read(&file).expect("read unchanged places"),
        contents.to_vec()
    );
}
