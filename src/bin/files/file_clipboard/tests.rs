use std::path::PathBuf;

use gpui_kit::ClipboardItem;

use super::{Error, Identity, Mode, Snapshot, cut::CutCache, fallback, protocol::Formats};

#[test]
fn file_formats_round_trip_intent_and_original_names() {
    let paths = vec![
        PathBuf::from("/tmp/a # percent% and\nnewline"),
        PathBuf::from("/tmp/中文.txt"),
    ];
    let snapshot = Snapshot::new(Mode::Cut, paths.clone()).expect("file selection");
    let formats = Formats::encode(&snapshot).expect("encode file formats");
    assert_eq!(
        Formats::decode_private(&formats.private).expect("private payload"),
        snapshot
    );
    let gnome = Formats::decode_gnome(&formats.gnome).expect("GNOME file list");
    assert_eq!(gnome.mode, Mode::Cut);
    assert_eq!(gnome.paths, paths);
    assert_eq!(
        Formats::kde_mode(&formats.kde_cut).expect("KDE cut marker"),
        Mode::Cut
    );
    let copied = Formats::decode_uris(&formats.uri_list, Mode::Copy).expect("URI list");
    assert_eq!(copied.mode, Mode::Copy);
    assert_eq!(copied.paths, paths);
    assert!(!copied.identity.is_owned());
}

#[test]
fn unrelated_text_remote_uris_and_invalid_intent_do_not_become_files() {
    let plain = ClipboardItem::new_string("file:///tmp/a\n".into());
    assert_eq!(
        fallback::Clipboard::decode(Some(&plain)).expect("ordinary text is not a file offer"),
        None
    );
    for uri in [
        "https://example.org/a",
        "file://server/tmp/a",
        "file:relative",
        "file:///tmp/a?query",
        "file:///tmp/a#fragment",
        "file:///tmp/a%00b",
        "/tmp/a",
    ] {
        assert!(
            Formats::decode_uris(uri.as_bytes(), Mode::Copy).is_err(),
            "{uri}"
        );
    }
    assert!(Formats::decode_gnome(b"move\nfile:///tmp/a\n").is_err());
    assert!(Formats::kde_mode(b"maybe").is_err());
    assert_eq!(
        Formats::kde_mode(b"0\r\n").expect("copy marker"),
        Mode::Copy
    );
    let local = Formats::decode_uris(b"# comment\r\nfile://localhost/tmp/a\r\n", Mode::Copy)
        .expect("local host");
    assert_eq!(local.paths, [PathBuf::from("/tmp/a")]);
}

#[test]
fn completed_cut_entries_only_change_the_unchanged_owned_snapshot() {
    let a = PathBuf::from("/tmp/a");
    let b = PathBuf::from("/tmp/b");
    let snapshot =
        Snapshot::new(Mode::Cut, vec![a.clone(), b.clone(), a.clone()]).expect("selection");
    assert_eq!(snapshot.paths, [a.clone(), b.clone()]);
    assert_eq!(
        snapshot.remaining_after(&snapshot, &[a.clone()]),
        Some(vec![b.clone()])
    );
    assert_eq!(
        snapshot.remaining_after(&snapshot, &[a.clone(), b.clone()]),
        Some(Vec::new())
    );
    assert_eq!(
        snapshot.remaining_after(&snapshot, &[PathBuf::from("/tmp/other")]),
        None
    );
    let new_copy =
        Snapshot::new(Mode::Cut, snapshot.paths.clone()).expect("same paths copied again");
    assert_ne!(snapshot.identity, new_copy.identity);
    assert_eq!(snapshot.remaining_after(&new_copy, &[a.clone()]), None);
    let foreign = Snapshot {
        mode: Mode::Cut,
        paths: snapshot.paths.clone(),
        identity: Identity(None),
    };
    assert_eq!(foreign.remaining_after(&foreign, &[a]), None);
    let copy = Snapshot::new(Mode::Copy, vec![b.clone()]).expect("copy");
    assert_eq!(copy.remaining_after(&copy, &[b]), None);
}

#[cfg(unix)]
#[test]
fn unix_non_utf8_paths_survive_uri_and_gpui_metadata_round_trips() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let path = PathBuf::from(OsString::from_vec(b"/tmp/raw-\xff%#\n".to_vec()));
    let snapshot = Snapshot::new(Mode::Copy, vec![path.clone()]).expect("raw path selection");
    let formats = Formats::encode(&snapshot).expect("encode raw paths");
    assert_eq!(
        Formats::decode_uris(&formats.uri_list, Mode::Copy)
            .expect("decode raw URI")
            .paths,
        [path]
    );
    let item = fallback::Clipboard::encode(&snapshot)
        .expect("GPUI item without touching system clipboard");
    assert_eq!(
        fallback::Clipboard::decode(Some(&item)).expect("GPUI own metadata"),
        Some(snapshot)
    );
    let invalid = PathBuf::from(OsString::from_vec(b"/tmp/nul\0".to_vec()));
    assert!(matches!(
        Snapshot::new(Mode::Copy, vec![invalid]),
        Err(Error::InvalidPath(_))
    ));
}

#[cfg(unix)]
#[test]
fn uri_parent_resolution_preserves_filesystem_meaning_without_following_the_leaf() {
    use std::{fs, os::unix::fs::symlink};
    let temp = tempfile::tempdir().expect("create fixture");
    let root = temp.path();
    fs::create_dir_all(root.join("actual/nested")).expect("create target tree");
    symlink(root.join("actual/nested"), root.join("alias")).expect("parent alias");
    symlink(root.join("missing"), root.join("actual/leaf")).expect("dangling leaf");
    let original = root.join("alias/../leaf");
    let snapshot =
        Snapshot::new(Mode::Cut, vec![original.clone()]).expect("selection through parent alias");
    let formats = Formats::encode(&snapshot).expect("URI resolves only the parent");
    assert_eq!(
        Formats::decode_uris(&formats.uri_list, Mode::Cut)
            .expect("decode URI")
            .paths,
        [root.join("actual/leaf")]
    );
    assert_eq!(
        Formats::decode_private(&formats.private)
            .expect("private raw identity")
            .paths,
        [original]
    );
    assert!(
        fs::symlink_metadata(root.join("actual/leaf"))
            .expect("leaf remains")
            .file_type()
            .is_symlink()
    );
}

#[test]
fn cut_cache_tracks_partial_pastes_and_newer_clipboard_writes() {
    let a = PathBuf::from("/tmp/cut-a");
    let b = PathBuf::from("/tmp/cut-b");
    let cut = Snapshot::new(Mode::Cut, vec![a.clone(), b.clone()]).expect("cut snapshot");
    let mut cache = CutCache::default();
    assert!(!cache.has_cut() && !cache.is_cut(&a));

    cache.record_write(&cut);
    assert!(cache.is_cut(&a) && cache.is_cut(&b));

    let copy = Snapshot::new(Mode::Copy, vec![a.clone()]).expect("copy snapshot");
    cache.record_write(&copy);
    assert!(!cache.has_cut(), "an ordinary copy replaces the cut intent");

    cache.record_write(&cut);
    cache.note_pasted(&cut, &[a.clone()]);
    assert!(!cache.is_cut(&a));
    assert!(
        cache.is_cut(&b),
        "an entry the paste did not move stays marked"
    );

    let newer = Snapshot::new(Mode::Cut, vec![a.clone()]).expect("newer cut");
    cache.note_pasted(&newer, &[a.clone()]);
    assert!(
        cache.is_cut(&b),
        "a paste of an older snapshot must not clear a newer intent"
    );

    cache.note_pasted(&cut, &[b.clone()]);
    assert!(!cache.has_cut(), "moving every source clears the intent");
}

#[test]
fn stale_cut_validation_cannot_override_a_newer_intent() {
    let a = PathBuf::from("/tmp/stale-a");
    let b = PathBuf::from("/tmp/stale-b");
    let cut = Snapshot::new(Mode::Cut, vec![a.clone()]).expect("cut snapshot");
    let mut cache = CutCache::default();
    cache.record_write(&cut);
    let (expected, generation) = cache.begin_refresh().expect("validation read");
    assert!(
        cache.begin_refresh().is_none(),
        "only one validation read is in flight"
    );
    let newer = Snapshot::new(Mode::Cut, vec![b.clone()]).expect("newer cut");
    cache.record_write(&newer);
    let (validating, validating_generation) = cache
        .begin_refresh()
        .expect("the newer intent validates at once");

    assert!(!cache.reconcile(generation, &expected, Ok(Some(cut))));
    assert!(cache.is_cut(&b) && !cache.is_cut(&a));
    assert!(
        cache.begin_refresh().is_none(),
        "a stale completion must not release the newer read"
    );
    assert!(!cache.reconcile(validating_generation, &validating, Ok(Some(newer))));
    assert!(cache.is_cut(&b));
}

#[test]
fn cut_validation_adopts_republished_paths_and_clears_replaced_content() {
    let a = PathBuf::from("/tmp/reconcile-a");
    let b = PathBuf::from("/tmp/reconcile-b");
    let cut = Snapshot::new(Mode::Cut, vec![a.clone(), b.clone()]).expect("cut snapshot");
    let mut cache = CutCache::default();
    cache.record_write(&cut);
    // A read started before the paste is still in flight when the kept paths
    // change; the changed intent must be able to validate on its own.
    let (stale, stale_generation) = cache.begin_refresh().expect("validation read");
    cache.note_pasted(&cut, &[a.clone()]);
    let (expected, generation) = cache.begin_refresh().expect("validation read");

    // A partial cut paste republishes the remaining paths with a fresh identity.
    let republished = Snapshot::new(Mode::Cut, vec![b.clone()]).expect("republished cut");
    assert!(!cache.reconcile(stale_generation, &stale, Ok(Some(cut.clone()))));
    assert!(
        cache.begin_refresh().is_none(),
        "the stale completion must not release the newer read"
    );
    cache.reconcile(generation, &expected, Ok(Some(republished)));
    assert!(cache.is_cut(&b) && !cache.is_cut(&a));

    let (expected, generation) = cache.begin_refresh().expect("validation read");
    assert!(cache.reconcile(generation, &expected, Ok(None)));
    assert!(!cache.has_cut(), "ordinary text ends the cut intent");

    cache.record_write(&cut);
    let (expected, generation) = cache.begin_refresh().expect("validation read");
    assert!(!cache.reconcile(generation, &expected, Err(Error::Changed)));
    assert!(
        cache.has_cut(),
        "an unreadable clipboard is not evidence of replacement"
    );
}
