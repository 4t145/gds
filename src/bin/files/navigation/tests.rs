use std::fs;

use super::*;

struct Fixture {
    root: tempfile::TempDir,
    a: PathBuf,
    b: PathBuf,
    c: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("create navigation fixture");
        let a = root.path().join("a");
        let b = root.path().join("b");
        let c = root.path().join("c");
        for path in [&a, &b, &c] {
            fs::create_dir(path).expect("create fixture directory");
            fs::write(path.join("selected"), b"data").expect("create selectable file");
        }
        Self { root, a, b, c }
    }

    fn navigation(&self) -> Navigation {
        Navigation::new(
            directory::read_directory(&self.a, ReadOptions::default())
                .expect("load initial fixture directory"),
        )
    }
}

fn apply(navigation: &mut Navigation, request: LoadRequest) {
    let result = request.load();
    assert_eq!(
        navigation
            .finish_load(&request, result)
            .expect("commit fixture load"),
        LoadOutcome::Applied
    );
}

fn open(navigation: &mut Navigation, path: &Path) {
    let request = navigation.prepare_open(path);
    apply(navigation, request);
}

#[test]
fn resolves_absolute_relative_and_home_paths_without_shell_expansion() {
    let fixture = Fixture::new();
    let home = fixture.root.path().join("home");

    assert_eq!(
        resolve_input(fixture.b.to_str().expect("UTF-8 fixture"), &fixture.a, None)
            .expect("absolute path"),
        fixture.b
    );
    assert_eq!(
        resolve_input("child", &fixture.a, None).expect("relative path"),
        fixture.a.join("child")
    );
    assert_eq!(
        resolve_input("~", &fixture.a, Some(&home)).expect("home path"),
        home
    );
    assert_eq!(
        resolve_input("~/Downloads", &fixture.a, Some(&home)).expect("home-relative path"),
        home.join("Downloads")
    );
    assert_eq!(
        resolve_input("~//Downloads", &fixture.a, Some(&home)).expect("home-relative slashes"),
        home.join("Downloads")
    );
    assert_eq!(
        resolve_input(" folder with spaces ", &fixture.a, None).expect("literal whitespace"),
        fixture.a.join(" folder with spaces ")
    );
    assert_eq!(
        resolve_input("$HOME/report:2026", &fixture.a, None).expect("literal shell syntax"),
        fixture.a.join("$HOME/report:2026")
    );
}

#[test]
fn rejects_uris_and_unsupported_home_inputs_explicitly() {
    let fixture = Fixture::new();
    for uri in [
        "sftp://server/path",
        "smb://server/share",
        "file:///tmp",
        "file:/tmp",
        "sftp:remote/path",
        "mailto:user@example.org",
    ] {
        assert!(matches!(
            resolve_input(uri, &fixture.a, None),
            Err(Error::UnsupportedUri { .. })
        ));
    }
    assert!(matches!(
        resolve_input("", &fixture.a, None),
        Err(Error::EmptyInput)
    ));
    assert!(matches!(
        resolve_input("~/docs", &fixture.a, None),
        Err(Error::MissingHome)
    ));
    assert!(matches!(
        resolve_input("~another/docs", &fixture.a, None),
        Err(Error::UnsupportedHomeExpansion { .. })
    ));
    assert!(matches!(
        resolve_input("child", Path::new("relative"), None),
        Err(Error::RelativeBase { .. })
    ));
    assert_eq!(
        resolve_input("./sftp://literal", &fixture.a, None).expect("local colon path"),
        fixture.a.join("./sftp://literal")
    );
}

#[test]
fn failed_navigation_preserves_location_history_selection_and_scroll() {
    let fixture = Fixture::new();
    let mut navigation = fixture.navigation();
    navigation
        .view_mut()
        .selected
        .push(fixture.a.join("selected"));
    navigation.view_mut().scroll_offset = 125.0;
    let before = navigation.view().clone();

    let request = navigation.prepare_open(fixture.root.path().join("missing"));
    assert_eq!(navigation.path(), fixture.a);
    assert!(navigation.is_loading());
    assert_eq!(navigation.history().len(), 1);
    assert!(matches!(
        navigation.finish_load(&request, request.load()),
        Err(Error::Load(_))
    ));

    assert_eq!(navigation.path(), fixture.a);
    assert_eq!(navigation.history().len(), 1);
    assert_eq!(navigation.history_index(), 0);
    assert_eq!(navigation.view(), &before);
    assert!(!navigation.is_loading());
}

#[test]
fn back_and_forward_restore_each_locations_view_state() {
    let fixture = Fixture::new();
    let mut navigation = fixture.navigation();
    navigation
        .view_mut()
        .selected
        .push(fixture.a.join("selected"));
    navigation.view_mut().scroll_offset = 15.0;
    open(&mut navigation, &fixture.b);
    assert_eq!(navigation.view(), &ViewState::default());
    navigation
        .view_mut()
        .selected
        .push(fixture.b.join("selected"));
    navigation.view_mut().scroll_offset = 80.0;

    let back = navigation.prepare_back().expect("back history exists");
    assert_eq!(navigation.path(), fixture.b);
    apply(&mut navigation, back);
    assert_eq!(navigation.path(), fixture.a);
    assert_eq!(navigation.view().selected, vec![fixture.a.join("selected")]);
    assert_eq!(navigation.view().scroll_offset, 15.0);
    assert!(!navigation.can_go_back());
    assert!(navigation.can_go_forward());

    let forward = navigation
        .prepare_forward()
        .expect("forward history exists");
    apply(&mut navigation, forward);
    assert_eq!(navigation.path(), fixture.b);
    assert_eq!(navigation.view().selected, vec![fixture.b.join("selected")]);
    assert_eq!(navigation.view().scroll_offset, 80.0);
    assert!(!navigation.can_go_forward());
}

#[test]
fn successful_new_branch_discards_forward_history_but_failed_branch_does_not() {
    let fixture = Fixture::new();
    let mut navigation = fixture.navigation();
    open(&mut navigation, &fixture.b);
    open(&mut navigation, &fixture.c);
    let back = navigation.prepare_back().expect("back history exists");
    apply(&mut navigation, back);

    let missing = navigation.prepare_open(fixture.root.path().join("missing"));
    assert!(navigation.finish_load(&missing, missing.load()).is_err());
    assert!(navigation.can_go_forward());
    assert_eq!(navigation.history().len(), 3);

    let branch = fixture.root.path().join("branch");
    fs::create_dir(&branch).expect("create alternate branch");
    open(&mut navigation, &branch);
    assert_eq!(
        navigation
            .history()
            .iter()
            .map(|location| location.path.clone())
            .collect::<Vec<_>>(),
        vec![fixture.a, fixture.b, branch]
    );
    assert!(!navigation.can_go_forward());
}

#[test]
fn failed_back_navigation_does_not_move_the_history_cursor() {
    let fixture = Fixture::new();
    let mut navigation = fixture.navigation();
    open(&mut navigation, &fixture.b);
    fs::remove_file(fixture.a.join("selected")).expect("remove old selectable file");
    fs::remove_dir(&fixture.a).expect("remove now-empty old directory");

    let request = navigation.prepare_back().expect("back history exists");
    assert!(navigation.finish_load(&request, request.load()).is_err());
    assert_eq!(navigation.path(), fixture.b);
    assert_eq!(navigation.history_index(), 1);
    assert!(navigation.can_go_back());
}

#[test]
fn failed_forward_navigation_does_not_move_the_history_cursor() {
    let fixture = Fixture::new();
    let mut navigation = fixture.navigation();
    open(&mut navigation, &fixture.b);
    let back = navigation.prepare_back().expect("back history exists");
    apply(&mut navigation, back);
    fs::remove_file(fixture.b.join("selected")).expect("remove future selectable file");
    fs::remove_dir(&fixture.b).expect("remove now-empty future directory");

    let request = navigation
        .prepare_forward()
        .expect("forward history exists");
    assert!(navigation.finish_load(&request, request.load()).is_err());
    assert_eq!(navigation.path(), fixture.a);
    assert_eq!(navigation.history_index(), 0);
    assert!(navigation.can_go_forward());
}

#[test]
fn refreshing_hidden_files_preserves_history_and_prunes_invisible_selection() {
    let fixture = Fixture::new();
    let hidden = fixture.a.join(".hidden");
    fs::write(&hidden, b"hidden").expect("create hidden file");
    let mut navigation = fixture.navigation();
    navigation.view_mut().scroll_offset = 90.0;
    let show = navigation.prepare_refresh(ReadOptions { show_hidden: true });
    assert!(!navigation.directory().options.show_hidden);
    apply(&mut navigation, show);
    assert!(navigation.directory().options.show_hidden);
    assert_eq!(navigation.directory().entries.len(), 2);
    navigation.view_mut().selected.push(hidden);

    let hide = navigation.prepare_refresh(ReadOptions::default());
    apply(&mut navigation, hide);
    assert_eq!(navigation.history().len(), 1);
    assert_eq!(navigation.view().scroll_offset, 90.0);
    assert!(navigation.view().selected.is_empty());
    open(&mut navigation, &fixture.a);
    assert_eq!(navigation.history().len(), 1);
}

#[test]
fn superseded_and_cancelled_background_results_cannot_replace_the_current_location() {
    let fixture = Fixture::new();
    let mut navigation = fixture.navigation();
    let older = navigation.prepare_open(&fixture.b);
    let latest = navigation.prepare_open(&fixture.c);
    assert!(
        !navigation.cancel_request(&older),
        "a disconnected older request cannot cancel a newer navigation"
    );
    assert_eq!(
        navigation
            .finish_load(&older, older.load())
            .expect("ignore superseded result"),
        LoadOutcome::Stale
    );
    assert_eq!(navigation.path(), fixture.a);
    assert!(navigation.is_loading());
    apply(&mut navigation, latest);
    assert_eq!(navigation.path(), fixture.c);

    let cancelled = navigation.prepare_open(&fixture.b);
    assert!(navigation.cancel_request(&cancelled));
    assert_eq!(
        navigation
            .finish_load(&cancelled, cancelled.load())
            .expect("ignore cancelled result"),
        LoadOutcome::Stale
    );
    assert_eq!(navigation.path(), fixture.c);
}

#[test]
fn tabs_have_independent_views_history_and_pending_requests() {
    let fixture = Fixture::new();
    let snapshot =
        directory::read_directory(&fixture.a, ReadOptions::default()).expect("load fixture");
    let mut first = Tab::new(snapshot.clone());
    let mut second = Tab::new(snapshot);
    first.pinned = true;
    first.navigation.view_mut().scroll_offset = 45.0;
    first
        .navigation
        .view_mut()
        .selected
        .push(fixture.a.join("selected"));
    let first_request = first.navigation.prepare_open(&fixture.b);
    let second_request = second.navigation.prepare_open(&fixture.c);

    assert_eq!(
        second
            .navigation
            .finish_load(&first_request, first_request.load())
            .expect("reject other tab's request"),
        LoadOutcome::Stale
    );
    assert!(second.navigation.is_loading());
    assert_eq!(second.navigation.view(), &ViewState::default());
    assert!(!second.pinned);
    apply(&mut first.navigation, first_request);
    apply(&mut second.navigation, second_request);
    assert_eq!(first.navigation.path(), fixture.b);
    assert_eq!(second.navigation.path(), fixture.c);
    assert_eq!(first.title(), "b");
}

#[cfg(unix)]
#[test]
fn opens_non_utf8_directory_paths_without_using_display_names() {
    use std::os::unix::ffi::OsStringExt;

    let fixture = Fixture::new();
    let name = std::ffi::OsString::from_vec(vec![b'd', 0xff, b'r']);
    let target = fixture.root.path().join(name);
    fs::create_dir(&target).expect("create non-UTF-8 directory");
    let mut navigation = fixture.navigation();
    open(&mut navigation, &target);
    assert_eq!(navigation.path(), target);
    assert_eq!(
        navigation.history().last().expect("new history entry").path,
        target
    );
}

#[cfg(unix)]
#[test]
fn parent_segments_after_symlinks_keep_filesystem_resolution_semantics() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let target = fixture.root.path().join("target");
    fs::create_dir_all(target.join("child")).expect("create symlink target");
    fs::create_dir(target.join("neighbor")).expect("create actual neighboring directory");
    fs::write(target.join("neighbor/landmark"), b"correct directory").expect("create landmark");
    symlink(target.join("child"), fixture.a.join("link")).expect("create directory symlink");
    let mut navigation = fixture.navigation();

    let request = navigation
        .prepare_input("link/../neighbor", None)
        .expect("resolve relative path");
    apply(&mut navigation, request);
    assert_eq!(navigation.directory().entries[0].name, "landmark");
}
