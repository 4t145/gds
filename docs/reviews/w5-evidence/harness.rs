extern crate self as nix;
pub mod libc { pub const O_NOFOLLOW: i32 = 0x20000; }
#[derive(Debug)] pub enum Error { Invalid(&'static str), Io(std::io::Error) }
impl From<std::io::Error> for Error { fn from(e: std::io::Error) -> Self { Self::Io(e) } }
#[path="paths.rs"] mod paths;
#[cfg(test)] mod tests {
 use super::*; use std::{fs, os::unix::fs::{PermissionsExt, symlink}};
 fn root() -> std::path::PathBuf { let p = std::env::temp_dir().join(format!("gds-w5-path-test-{}-{:?}",std::process::id(),std::thread::current().id())); fs::create_dir_all(&p).unwrap(); fs::set_permissions(&p,fs::Permissions::from_mode(0o700)).unwrap(); p }
 #[test] fn creates_private_cache() { let r=root(); let p=r.join("private/db"); paths::prepare(&p).unwrap(); assert_eq!(fs::metadata(&p).unwrap().permissions().mode()&0o777,0o600); assert_eq!(fs::metadata(p.parent().unwrap()).unwrap().permissions().mode()&0o777,0o700); assert_eq!(fs::metadata(&p).unwrap().len(),0); }
 #[test] fn rejects_public_parent() { let r=root(); fs::set_permissions(&r,fs::Permissions::from_mode(0o755)).unwrap(); assert!(paths::prepare(&r.join("db")).is_err()); }
 #[test] fn rejects_symlink_ancestor() { let r=root(); let target=r.join("target"); fs::create_dir(&target).unwrap(); symlink(&target,r.join("link")).unwrap(); assert!(paths::prepare(&r.join("link/private/db")).is_err()); }
 #[test] fn rejects_database_symlink() { let r=root(); fs::write(r.join("target"),"secret").unwrap(); symlink(r.join("target"),r.join("db")).unwrap(); assert!(paths::prepare(&r.join("db")).is_err()); assert_eq!(fs::read(r.join("target")).unwrap(),b"secret"); }
 #[test] fn rejects_public_database_and_hardlink() { let r=root(); let p=r.join("db"); fs::write(&p,"preserve").unwrap(); fs::set_permissions(&p,fs::Permissions::from_mode(0o644)).unwrap(); assert!(paths::prepare(&p).is_err()); fs::set_permissions(&p,fs::Permissions::from_mode(0o600)).unwrap(); fs::hard_link(&p,r.join("link")).unwrap(); assert!(paths::prepare(&p).is_err()); assert_eq!(fs::read(&p).unwrap(),b"preserve"); }
 #[test] fn rejects_public_sidecar() { let r=root(); let p=r.join("db"); fs::write(r.join("db-wal"),"secret").unwrap(); fs::set_permissions(r.join("db-wal"),fs::Permissions::from_mode(0o644)).unwrap(); assert!(paths::prepare(&p).is_err()); }
 #[test] fn accepts_existing_private_database_without_truncation() { let r=root(); let p=r.join("db"); fs::write(&p,"preserve").unwrap(); fs::set_permissions(&p,fs::Permissions::from_mode(0o600)).unwrap(); paths::prepare(&p).unwrap(); assert_eq!(fs::read(&p).unwrap(),b"preserve"); }
}
