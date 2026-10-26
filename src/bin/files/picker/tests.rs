use super::*;

#[test]
fn directory_picker_accepts_multiple_but_rejects_file_only_options() {
    let arguments = parse(["--pick-directory", "--multiple", "/tmp"].map(OsString::from))
        .expect("parse directory picker");
    let options = arguments.picker.expect("picker options");
    assert!(options.directory);
    assert!(options.multiple);
    assert_eq!(arguments.directory, Some(PathBuf::from("/tmp")));
    for arguments in [
        vec!["--pick-file", "--pick-directory"],
        vec!["--multiple"],
        vec!["--pick-directory", "--filter", "*.txt"],
        vec!["--pick-directory", "--read-only"],
    ] {
        assert!(parse(arguments.into_iter().map(OsString::from)).is_err());
    }
    let file = parse(["--pick-file"].map(OsString::from))
        .expect("parse file picker")
        .picker
        .expect("picker options");
    assert!(!file.directory);
    assert!(!file.multiple);
    assert!(
        parse(["--pick-file", "--multiple"].map(OsString::from))
            .expect("parse multiple file picker")
            .picker
            .expect("picker options")
            .multiple
    );
}

#[test]
#[cfg(unix)]
fn directory_results_preserve_single_protocol_and_multiple_raw_paths() {
    use std::os::unix::ffi::OsStringExt as _;
    let paths = [
        PathBuf::from("/tmp/one folder"),
        PathBuf::from(OsString::from_vec(b"/tmp/\xff".to_vec())),
    ];
    let single: serde_json::Value = serde_json::from_slice(
        &selection_line(&paths[..1], false, 0).expect("serialize single directory"),
    )
    .expect("parse single directory");
    assert_eq!(single["path"], "/tmp/one folder");
    assert!(single.get("selections").is_none());
    let multiple: serde_json::Value =
        serde_json::from_slice(&selection_line(&paths, false, 0).expect("serialize directories"))
            .expect("parse directories");
    assert_eq!(multiple["selections"][0], single);
    assert!(multiple["selections"][1]["path"].is_null());
    assert_eq!(
        multiple["selections"][1]["path_os"]["Unix"],
        serde_json::json!(b"/tmp/\xff".to_vec())
    );
}
