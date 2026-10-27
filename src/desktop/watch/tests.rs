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
