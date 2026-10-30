use super::*;
use std::{fs, time::Duration};

fn changed(receiver: &async_channel::Receiver<()>) {
    futures_lite::future::block_on(futures_lite::future::race(
        async {
            receiver.recv().await.expect("live file watcher");
        },
        async {
            async_io::Timer::after(Duration::from_secs(3)).await;
            panic!("file change was not observed");
        },
    ));
}

fn drain_until_quiet(receiver: &async_channel::Receiver<()>) {
    futures_lite::future::block_on(async {
        while futures_lite::future::race(
            async {
                receiver.recv().await.expect("live file watcher");
                true
            },
            async {
                async_io::Timer::after(Duration::from_millis(150)).await;
                false
            },
        )
        .await
        {}
    });
}

#[test]
fn ignores_unrelated_files_in_watched_directory() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("palette.json");
    fs::write(&target, "initial").unwrap();
    let (sender, receiver) = async_channel::bounded(1);
    let mut sources = Sources::new(sender);
    sources.set_paths(vec![target.clone()]).unwrap();
    fs::write(&target, "observed").unwrap();
    changed(&receiver);
    drain_until_quiet(&receiver);
    fs::write(directory.path().join("unrelated.json"), "ignored").unwrap();
    futures_lite::future::block_on(futures_lite::future::race(
        async {
            receiver.recv().await.unwrap();
            panic!("unrelated event was forwarded");
        },
        async {
            async_io::Timer::after(Duration::from_millis(150)).await;
        },
    ));
}

#[test]
fn watches_parent_symlink_replacement_and_new_target_changes() {
    parent_symlink_replacement(true);
}

#[test]
fn watches_missing_file_after_parent_symlink_replacement() {
    parent_symlink_replacement(false);
}

fn parent_symlink_replacement(file_exists: bool) {
    let directory = tempfile::tempdir().expect("managed directory sources");
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    if file_exists {
        for target in [&first, &second] {
            fs::write(target.join("palette.json"), "initial").unwrap();
        }
    }
    let managed = directory.path().join("managed");
    std::os::unix::fs::symlink(&first, &managed).unwrap();
    let (sender, receiver) = async_channel::bounded(1);
    let mut sources = Sources::new(sender);
    sources
        .set_paths(vec![managed.join("palette.json")])
        .unwrap();

    let replacement = directory.path().join("replacement");
    std::os::unix::fs::symlink(&second, &replacement).unwrap();
    fs::rename(&replacement, &managed).unwrap();
    changed(&receiver);
    sources
        .refresh()
        .expect("follow replacement parent symlink");
    drain_until_quiet(&receiver);
    fs::write(second.join("palette.json"), "updated").unwrap();
    changed(&receiver);
}

#[test]
fn watches_missing_sources_and_atomic_replacements_through_managed_links() {
    let directory = tempfile::tempdir().expect("theme sources");
    let path = directory.path().join("created/palette.json");
    let (sender, receiver) = async_channel::bounded(1);
    let mut sources = Sources::new(sender);
    sources
        .set_paths(vec![path.clone()])
        .expect("watch missing source");
    fs::create_dir(path.parent().expect("source parent")).expect("create source directory");
    changed(&receiver);
    sources.refresh().expect("follow created directory");
    fs::write(&path, "first").expect("create palette");
    changed(&receiver);

    let managed = directory.path().join("managed.json");
    std::os::unix::fs::symlink(&path, &managed).expect("managed palette");
    sources
        .set_paths(vec![managed])
        .expect("watch link and resolved source");
    while receiver.try_recv().is_ok() {}
    let replacement = path.with_extension("new");
    fs::write(&replacement, "second").expect("new palette");
    fs::rename(replacement, &path).expect("atomic palette replacement");
    changed(&receiver);
}
