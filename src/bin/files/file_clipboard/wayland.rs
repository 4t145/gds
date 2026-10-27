use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use wl_clipboard_rs::{copy, paste};

use super::{
    Error, Mode, Snapshot,
    protocol::{Formats, GDS_FILES, GNOME_FILES, KDE_CUT, MAX_PAYLOAD_BYTES, URI_LIST},
};

const READ_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct Clipboard;

impl Clipboard {
    /// # Errors
    /// Reports payload encoding or Wayland offer failures.
    pub fn write(snapshot: &Snapshot) -> Result<(), Error> {
        Self::publish(Formats::encode(snapshot)?)
    }

    /// # Errors
    /// Propagates clipboard protocol and publication failures.
    fn publish(formats: Formats) -> Result<(), Error> {
        let sources = [
            (GDS_FILES, formats.private),
            (URI_LIST, formats.uri_list),
            (GNOME_FILES, formats.gnome),
            (KDE_CUT, formats.kde_cut),
        ]
        .into_iter()
        .map(|(mime, bytes)| copy::MimeSource {
            source: copy::Source::Bytes(bytes.into_boxed_slice()),
            mime_type: copy::MimeType::Specific(mime.to_owned()),
        })
        .collect();
        let mut options = copy::Options::new();
        options
            .clipboard(copy::ClipboardType::Regular)
            .omit_additional_text_mime_types(true);
        // The library owns the serving thread. This returns after preparation,
        // not when another application eventually replaces the clipboard.
        options.copy_multi(sources)?;
        Ok(())
    }

    /// # Errors
    /// Reports malformed/changing offers, protocol errors or a stalled data source.
    pub fn read() -> Result<Option<Snapshot>, Error> {
        let deadline = Instant::now() + READ_TIMEOUT;
        if let Some(bytes) = Self::read_mime(GDS_FILES, deadline)? {
            return Formats::decode_private(&bytes).map(Some);
        }
        if let Some(bytes) = Self::read_mime(GNOME_FILES, deadline)? {
            return Formats::decode_gnome(&bytes).map(Some);
        }
        let Some(uris) = Self::read_mime(URI_LIST, deadline)? else {
            return Ok(None);
        };
        let marker = Self::read_mime(KDE_CUT, deadline)?;
        // The library obtains each MIME on a new connection. Detect an intervening
        // replacement before combining the KDE intent with its URI list.
        if Self::read_mime(URI_LIST, deadline)?.as_ref() != Some(&uris) {
            return Err(Error::Changed);
        }
        let mode = marker
            .as_deref()
            .map(Formats::kde_mode)
            .transpose()?
            .unwrap_or(Mode::Copy);
        Formats::decode_uris(&uris, mode).map(Some)
    }

    /// # Errors
    /// Reports failures reading or updating the still-owned clipboard.
    pub fn finish_cut(expected: &Snapshot, succeeded: &[PathBuf]) -> Result<(), Error> {
        let Some(current) = Self::read_owned()? else {
            return Ok(());
        };
        let Some(remaining) = expected.remaining_after(&current, succeeded) else {
            return Ok(());
        };
        let replacement = if remaining.is_empty() {
            None
        } else {
            Some(Formats::encode(&Snapshot::new(Mode::Cut, remaining)?)?)
        };
        // Encoding can resolve parent traversal; do not trust an offer read before it.
        if Self::read_owned()?.as_ref() != Some(&current) {
            return Ok(());
        }
        if let Some(formats) = replacement {
            Self::publish(formats)?;
        } else {
            copy::clear(copy::ClipboardType::Regular, copy::Seat::All)?;
        }
        Ok(())
    }

    /// # Errors
    /// Rejects malformed private data and transport failures.
    fn read_owned() -> Result<Option<Snapshot>, Error> {
        Self::read_mime(GDS_FILES, Instant::now() + READ_TIMEOUT)?
            .map(|bytes| Formats::decode_private(&bytes))
            .transpose()
    }

    /// # Errors
    /// Reports protocol, I/O, timeout and payload-limit failures.
    fn read_mime(mime: &str, deadline: Instant) -> Result<Option<Vec<u8>>, Error> {
        let result = paste::get_contents(
            paste::ClipboardType::Regular,
            paste::Seat::Unspecified,
            paste::MimeType::Specific(mime),
        );
        let (pipe, _) = match result {
            Ok(result) => result,
            Err(paste::Error::ClipboardEmpty | paste::Error::NoMimeType) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        gds::desktop::clipboard::read_pipe(pipe, deadline, MAX_PAYLOAD_BYTES)
            .map(Some)
            .map_err(|error| match error {
                gds::desktop::clipboard::Error::Io(error) => Error::Io(error),
                gds::desktop::clipboard::Error::Timeout => Error::Timeout,
                gds::desktop::clipboard::Error::TooLarge => Error::TooLarge,
            })
    }
}
