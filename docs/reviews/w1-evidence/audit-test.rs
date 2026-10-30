#[cfg(test)]
mod audit_reproduction {
    use super::*;
    #[test]
    fn late_child_is_deleted_without_being_copied() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destdir = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destdir).unwrap();
        fs::write(source.join("original"), b"old").unwrap();
        let item = Item { source: source.clone(), destination: destdir.join("source"), destination_committed: false, result: Ok(()) };
        let prepared = Prepared::new(&item).unwrap();
        let stage = Staging::new(&prepared.directory).unwrap();
        let node = Node { source: prepared.source.clone(), destination: stage.payload() };
        node.copy_root().unwrap();
        fs::write(source.join("late-important"), b"NEW UNCOPIED DATA").unwrap();
        Prepared::rename_no_replace(&node.destination, &prepared.destination).unwrap();
        stage.finish(Ok(())).unwrap();
        prepared.remove_source().unwrap();
        assert!(!source.exists());
        assert!(!prepared.destination.join("late-important").exists());
        assert_eq!(fs::read(prepared.destination.join("original")).unwrap(), b"old");
        println!("CONFIRMED: late-important absent from both source and destination after successful removal");
    }
}
