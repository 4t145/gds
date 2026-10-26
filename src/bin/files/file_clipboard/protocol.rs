use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use url::Url;

use super::{Error, Identity, Mode, Snapshot};

pub(super) const URI_LIST: &str = "text/uri-list";
pub(super) const GNOME_FILES: &str = "x-special/gnome-copied-files";
pub(super) const KDE_CUT: &str = "application/x-kde-cutselection";
pub(super) const GDS_FILES: &str = "application/vnd.gds.files+json";
pub(super) const METADATA_PREFIX: &str = "gds-files-v1:";
pub(super) const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
const VERSION: u8 = 1;

#[derive(Serialize, Deserialize)]
struct Payload {
    version: u8,
    identity: String,
    mode: Mode,
    paths: Vec<Vec<u8>>,
}

pub(super) struct Formats {
    pub private: Vec<u8>,
    pub uri_list: Vec<u8>,
    pub gnome: Vec<u8>,
    pub kde_cut: Vec<u8>,
}

impl Formats {
    /// # Errors
    /// Reports invalid paths, serialization failures or oversized payloads.
    pub fn encode(snapshot: &Snapshot) -> Result<Self, Error> {
        let private = Self::encode_private(snapshot)?;
        let uri_list = Self::encode_uris(&snapshot.paths)?;
        let mut gnome = match snapshot.mode {
            Mode::Copy => b"copy\n".to_vec(),
            Mode::Cut => b"cut\n".to_vec(),
        };
        gnome.extend(uri_list.iter().copied().filter(|byte| *byte != b'\r'));
        let kde_cut = match snapshot.mode {
            Mode::Copy => b"0".to_vec(),
            Mode::Cut => b"1".to_vec(),
        };
        if [&private, &uri_list, &gnome]
            .iter()
            .any(|bytes| bytes.len() > MAX_PAYLOAD_BYTES)
        {
            return Err(Error::TooLarge);
        }
        Ok(Self {
            private,
            uri_list,
            gnome,
            kde_cut,
        })
    }

    /// # Errors
    /// Requires a GDS-owned identity and serializable platform paths.
    pub fn encode_private(snapshot: &Snapshot) -> Result<Vec<u8>, Error> {
        let identity = snapshot
            .identity
            .0
            .clone()
            .ok_or_else(|| Error::InvalidPayload("cannot publish a foreign identity".into()))?;
        let paths = snapshot
            .paths
            .iter()
            .map(|path| Self::path_bytes(path))
            .collect::<Result<_, _>>()?;
        let bytes = serde_json::to_vec(&Payload {
            version: VERSION,
            identity,
            mode: snapshot.mode,
            paths,
        })?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(Error::TooLarge);
        }
        Ok(bytes)
    }

    /// # Errors
    /// Rejects invalid versions, identities, paths and oversized metadata.
    pub fn decode_private(bytes: &[u8]) -> Result<Snapshot, Error> {
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(Error::TooLarge);
        }
        let payload: Payload = serde_json::from_slice(bytes)?;
        if payload.version != VERSION || payload.identity.is_empty() {
            return Err(Error::InvalidPayload(
                "unsupported GDS clipboard version or missing identity".into(),
            ));
        }
        let paths = payload
            .paths
            .into_iter()
            .map(Self::path_from_bytes)
            .collect::<Result<_, _>>()?;
        Ok(Snapshot {
            mode: payload.mode,
            paths: Snapshot::validate_paths(paths)?,
            identity: Identity(Some(payload.identity)),
        })
    }

    /// # Errors
    /// Reports paths which cannot become local file URLs or resolve parent traversal.
    pub fn encode_uris(paths: &[PathBuf]) -> Result<Vec<u8>, Error> {
        let mut output = String::new();
        for path in paths {
            // URL parsers normalize '..'; resolve only its parent when necessary
            // so an alias/../child retains filesystem meaning, never following the leaf.
            let resolved;
            let wire_path = if path.components().any(|part| part == Component::ParentDir) {
                let parent = path
                    .parent()
                    .ok_or_else(|| Error::InvalidPath(path.clone()))?;
                let name = path
                    .file_name()
                    .ok_or_else(|| Error::InvalidPath(path.clone()))?;
                resolved = parent.canonicalize()?.join(name);
                &resolved
            } else {
                path
            };
            let uri =
                Url::from_file_path(wire_path).map_err(|_| Error::InvalidPath(path.clone()))?;
            output.push_str(uri.as_str());
            output.push_str("\r\n");
            if output.len() > MAX_PAYLOAD_BYTES {
                return Err(Error::TooLarge);
            }
        }
        Ok(output.into_bytes())
    }

    /// # Errors
    /// Rejects invalid, remote, non-file or empty URI lists.
    pub fn decode_uris(bytes: &[u8], mode: Mode) -> Result<Snapshot, Error> {
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(Error::TooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| {
            Error::InvalidPayload("URI list must be UTF-8 with percent-encoded path bytes".into())
        })?;
        let paths = text
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(Self::local_file_uri)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Snapshot {
            mode,
            paths: Snapshot::validate_paths(paths)?,
            identity: Identity(None),
        })
    }

    /// # Errors
    /// Requires a copy/cut intent and a valid local URI list.
    pub fn decode_gnome(bytes: &[u8]) -> Result<Snapshot, Error> {
        let mut lines = bytes.splitn(2, |byte| *byte == b'\n');
        let intent = lines.next().expect("splitn yields at least one slice");
        let Some(uris) = lines.next() else {
            return Err(Error::InvalidPayload(
                "GNOME file clipboard has no intent line".into(),
            ));
        };
        let mode = match intent.strip_suffix(b"\r").unwrap_or(intent) {
            b"copy" => Mode::Copy,
            b"cut" => Mode::Cut,
            _ => {
                return Err(Error::InvalidPayload(
                    "GNOME file clipboard intent must be copy or cut".into(),
                ));
            }
        };
        Self::decode_uris(uris, mode)
    }

    /// # Errors
    /// Requires a recognized KDE copy/cut marker.
    pub fn kde_mode(bytes: &[u8]) -> Result<Mode, Error> {
        let marker = bytes
            .trim_ascii()
            .strip_suffix(b"\0")
            .unwrap_or(bytes.trim_ascii());
        match marker {
            b"0" => Ok(Mode::Copy),
            b"1" => Ok(Mode::Cut),
            _ => Err(Error::InvalidPayload(
                "KDE cut marker must be 0 or 1".into(),
            )),
        }
    }

    /// # Errors
    /// Rejects non-local, relative or parameterized file URLs.
    fn local_file_uri(text: &str) -> Result<PathBuf, Error> {
        let uri = Url::parse(text).map_err(|_| Error::InvalidUri(text.into()))?;
        let absolute = text
            .split_once(':')
            .is_some_and(|(_, rest)| rest.starts_with('/'));
        if text.as_bytes().contains(&0)
            || !absolute
            || uri.scheme() != "file"
            || uri.host_str().is_some_and(|host| host != "localhost")
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.port().is_some()
            || uri.query().is_some()
            || uri.fragment().is_some()
        {
            return Err(Error::InvalidUri(text.into()));
        }
        uri.to_file_path()
            .map_err(|_| Error::InvalidUri(text.into()))
    }

    /// # Errors
    /// Non-Unix paths must be representable without lossy text conversion.
    fn path_bytes(path: &Path) -> Result<Vec<u8>, Error> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Ok(path.as_os_str().as_bytes().to_vec())
        }
        #[cfg(not(unix))]
        path.to_str()
            .map(|text| text.as_bytes().to_vec())
            .ok_or_else(|| Error::InvalidPath(path.to_owned()))
    }

    /// # Errors
    /// Non-Unix platforms require valid UTF-8 path data.
    fn path_from_bytes(bytes: Vec<u8>) -> Result<PathBuf, Error> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            Ok(PathBuf::from(OsString::from_vec(bytes)))
        }
        #[cfg(not(unix))]
        String::from_utf8(bytes)
            .map(PathBuf::from)
            .map_err(|_| Error::InvalidPayload("path is not valid platform text".into()))
    }
}
