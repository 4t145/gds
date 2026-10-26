use std::path::PathBuf;

use gpui_kit::{App, ClipboardEntry, ClipboardItem, ExternalPaths, Task};

use super::{
    Error, Mode, Snapshot,
    protocol::{Formats, METADATA_PREFIX},
};

pub(super) struct Clipboard;

impl Clipboard {
    /// # Errors
    /// Rejects invalid selections and metadata encoding failures.
    pub fn write(mode: Mode, paths: Vec<PathBuf>, cx: &mut App) -> Result<Snapshot, Error> {
        let snapshot = Snapshot::new(mode, paths)?;
        cx.write_to_clipboard(Self::encode(&snapshot)?);
        Ok(snapshot)
    }

    /// # Errors
    /// The task reports platform read or GDS metadata failures.
    pub fn read(cx: &mut App) -> Task<Result<Option<Snapshot>, Error>> {
        let read = cx.read_from_clipboard_async();
        cx.spawn(async move |_| {
            let item = read.await?;
            Self::decode(item.as_ref())
        })
    }

    /// # Errors
    /// The task reports platform read or metadata encoding failures.
    pub fn finish_cut(
        expected: Snapshot,
        succeeded: Vec<PathBuf>,
        cx: &mut App,
    ) -> Task<Result<(), Error>> {
        let read = cx.read_from_clipboard_async();
        cx.spawn(async move |cx| {
            let item = read.await?;
            let Some(current) = Self::decode(item.as_ref())? else {
                return Ok(());
            };
            let Some(remaining) = expected.remaining_after(&current, &succeeded) else {
                return Ok(());
            };
            let item = if remaining.is_empty() {
                ClipboardItem::new_string(String::new())
            } else {
                Self::encode(&Snapshot::new(Mode::Cut, remaining)?)?
            };
            cx.update(|cx| cx.write_to_clipboard(item));
            Ok(())
        })
    }

    /// # Errors
    /// Rejects invalid private metadata or file URL representations.
    pub fn encode(snapshot: &Snapshot) -> Result<ClipboardItem, Error> {
        let private = String::from_utf8(Formats::encode_private(snapshot)?)
            .expect("JSON encoder always produces UTF-8");
        let text = String::from_utf8(Formats::encode_uris(&snapshot.paths)?)
            .expect("URL serialization always produces UTF-8");
        // GPUI's X11 metadata cache is keyed by text. Include the per-copy identity
        // in that text so separately copied ordinary URI text cannot reuse old intent.
        let identity = snapshot
            .identity
            .0
            .as_deref()
            .expect("private encoding requires an identity");
        let text = format!("# {METADATA_PREFIX}{identity}\r\n{text}");
        let mut item =
            ClipboardItem::new_string_with_metadata(text, format!("{METADATA_PREFIX}{private}"));
        item.entries
            .push(ClipboardEntry::ExternalPaths(ExternalPaths(
                snapshot.paths.iter().cloned().collect(),
            )));
        Ok(item)
    }

    /// # Errors
    /// Rejects malformed GDS data or mismatched embedded paths.
    pub fn decode(item: Option<&ClipboardItem>) -> Result<Option<Snapshot>, Error> {
        let Some(item) = item else {
            return Ok(None);
        };
        let Some(private) = item.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::String(text) => text.metadata.as_deref()?.strip_prefix(METADATA_PREFIX),
            _ => None,
        }) else {
            return Ok(None);
        };
        let snapshot = Formats::decode_private(private.as_bytes())?;
        let matches_paths = item.entries().iter().any(|entry| match entry {
            ClipboardEntry::ExternalPaths(paths) => paths.paths() == snapshot.paths,
            _ => false,
        });
        let marker = format!(
            "# {METADATA_PREFIX}{}",
            snapshot
                .identity
                .0
                .as_deref()
                .expect("decoded private identity")
        );
        let matches_identity = item.entries().iter().any(|entry| match entry {
            ClipboardEntry::String(text) => text.text.lines().next() == Some(marker.as_str()),
            _ => false,
        });
        if !matches_paths || !matches_identity {
            return Err(Error::InvalidPayload(
                "GDS metadata and clipboard contents disagree".into(),
            ));
        }
        Ok(Some(snapshot))
    }
}
