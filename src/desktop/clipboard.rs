//! Bounded clipboard pipe reads. MIME negotiation and payload meaning belong to each app.

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use std::{
    io::{self, Read},
    os::fd::AsFd,
    time::Instant,
};

const BUFFER_SIZE: usize = 8192;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("clipboard I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("clipboard source timed out")]
    Timeout,
    #[error("clipboard payload exceeds the supported size")]
    TooLarge,
}

/// Read a clipboard pipe until EOF with one deadline and a caller-selected size limit.
/// Call from a background executor.
///
/// # Errors
/// Reports a stalled source, oversized payload or an I/O error. Interrupted calls retry
/// within the original deadline.
pub fn read_pipe(
    mut pipe: impl Read + AsFd,
    deadline: Instant,
    limit: usize,
) -> Result<Vec<u8>, Error> {
    let mut contents = Vec::new();
    let mut buffer = [0u8; BUFFER_SIZE];
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error::Timeout)?;
        let timeout = Timespec {
            tv_sec: remaining.as_secs() as _,
            tv_nsec: remaining.subsec_nanos() as _,
        };
        let mut fds = [PollFd::new(&pipe, PollFlags::IN)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) => return Err(Error::Timeout),
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(io::Error::from(error).into()),
        }
        let count = match pipe.read(&mut buffer) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        if count == 0 {
            return Ok(contents);
        }
        if count > limit.saturating_sub(contents.len()) {
            return Err(Error::TooLarge);
        }
        contents.extend_from_slice(&buffer[..count]);
    }
}

#[cfg(test)]
mod tests;
