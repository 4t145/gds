#[path="/home/atlas/Github/4t145/gds/.agents/worktrees/45177b39-414c-43de-be5f-79096d3c51b8-w6/src/desktop/watch.rs"]
mod watch;

pub fn missing_file_is_created_after_parent_symlink_switch() {
    use std::{fs, time::Duration};
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    let new = root.path().join("new");
    fs::create_dir(&old).unwrap();
    fs::create_dir(&new).unwrap();
    let link = root.path().join("current");
    std::os::unix::fs::symlink(&old, &link).unwrap();
    let (tx, rx) = async_channel::bounded(1);
    let mut sources = watch::Sources::new(tx);
    sources.set_paths(vec![link.join("missing.json")]).unwrap();
    let replacement = root.path().join("replacement");
    std::os::unix::fs::symlink(&new, &replacement).unwrap();
    fs::rename(replacement, link).unwrap();
    let changed = || futures_lite::future::block_on(futures_lite::future::race(
        async { rx.recv().await.unwrap(); true },
        async { async_io::Timer::after(Duration::from_secs(3)).await; false },
    ));
    assert!(changed(), "parent switch must wake");
    sources.refresh().unwrap();
    // Wait for a quiet notification interval so delayed rename events cannot
    // satisfy the subsequent file-creation observation.
    while futures_lite::future::block_on(futures_lite::future::race(
        async { rx.recv().await.unwrap(); true },
        async { async_io::Timer::after(Duration::from_millis(150)).await; false },
    )) {}
    fs::write(new.join("missing.json"), "created").unwrap();
    assert!(changed(), "new target file creation must wake");
}
