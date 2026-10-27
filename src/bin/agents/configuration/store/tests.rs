use super::*;

#[test]
fn frontend_updates_preserve_other_sections_and_reject_stale_edits() {
    let directory = tempfile::tempdir().expect("settings directory");
    let store = Store::at(directory.path().join("settings.json"));
    std::fs::write(
        store.path(),
        r#"{"daemon":{"unknown":"server-owned"},"future":true}"#,
    )
    .expect("legacy document");
    let mut appearance = store.load().expect("frontend ignores daemon schema");
    let mut connections = store.load().expect("second editor");
    let candidate = Appearance {
        ui_font_size: Some(19.),
        ..Default::default()
    };
    store
        .save_appearance(&mut appearance, candidate.clone())
        .expect("appearance save");
    let mut frontend = connections.settings.frontend.clone();
    frontend.connections[0].name = "My local service".into();
    store
        .save_frontend(&mut connections, frontend)
        .expect("independent section merges");
    let raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(store.path()).expect("settings")).expect("JSON");
    assert_eq!(raw["daemon"]["unknown"], "server-owned");
    assert_eq!(raw["future"], true);
    assert_eq!(
        store.load().expect("merged document").settings.appearance,
        candidate
    );
    assert!(matches!(
        store.save_appearance(&mut appearance, Appearance::default()),
        Ok(())
    ));
    assert!(matches!(
        store.save_appearance(&mut connections, candidate),
        Err(Error::Conflict { .. })
    ));
}

#[cfg(unix)]
#[test]
fn managed_settings_can_be_read_but_never_replaced() {
    let directory = tempfile::tempdir().expect("settings directory");
    let source = directory.path().join("managed.json");
    let path = directory.path().join("settings.json");
    std::fs::write(&source, "{}").expect("managed source");
    std::os::unix::fs::symlink(&source, &path).expect("managed link");
    let store = Store::at(path.clone());
    let mut document = store.load().expect("read managed settings");
    assert!(
        store
            .save_appearance(&mut document, Appearance::default())
            .is_err()
    );
    assert!(path.is_symlink());
    assert_eq!(std::fs::read_to_string(source).expect("source"), "{}");
}
