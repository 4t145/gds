use super::*;
use std::{io::Write, os::unix::net::UnixStream, time::Duration};

#[test]
fn pipe_reads_require_eof_and_enforce_size_and_deadline() {
    let (reader, mut writer) = UnixStream::pair().expect("local pipe");
    writer.write_all(b"abcd").expect("offer payload");
    drop(writer);
    assert_eq!(
        read_pipe(reader, Instant::now() + Duration::from_secs(1), 4).expect("complete offer"),
        b"abcd"
    );

    let (reader, mut writer) = UnixStream::pair().expect("local pipe");
    writer.write_all(b"abcde").expect("oversized offer");
    assert!(matches!(
        read_pipe(reader, Instant::now() + Duration::from_secs(1), 4),
        Err(Error::TooLarge)
    ));

    let (reader, _writer) = UnixStream::pair().expect("stalled offer");
    assert!(matches!(
        read_pipe(reader, Instant::now() + Duration::from_millis(10), 4),
        Err(Error::Timeout)
    ));
}
