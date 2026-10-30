mod terminal {
 mod backend {
  #[path = "/home/atlas/Github/4t145/gds/.agents/worktrees/45177b39-414c-43de-be5f-79096d3c51b8-w8/src/terminal/backend/shell.rs"] mod shell;
  #[path = "/home/atlas/Github/4t145/gds/.agents/worktrees/45177b39-414c-43de-be5f-79096d3c51b8-w8/src/terminal/backend/unix.rs"] mod unix;
  pub(crate) use unix::{Backend, Wake};
 }
 #[path = "/home/atlas/Github/4t145/gds/.agents/worktrees/45177b39-414c-43de-be5f-79096d3c51b8-w8/src/terminal/session.rs"] pub mod session;
 #[path = "/home/atlas/Github/4t145/gds/.agents/worktrees/45177b39-414c-43de-be5f-79096d3c51b8-w8/src/terminal/directory.rs"] mod directory;
}
use terminal::session::{Session, SpawnOptions};
use std::time::{Duration, Instant};
fn screen(s: &Session) -> String {
 use alacritty_terminal::grid::Dimensions;
 s.with_term(|t| { let grid=t.grid(); let mut text=String::new(); for row in 0..grid.screen_lines() { for col in 0..grid.columns() { text.push(grid[alacritty_terminal::index::Line(row as i32)][alacritty_terminal::index::Column(col)].c); } } text })
}
fn until(mut predicate: impl FnMut()->bool) { let end=Instant::now()+Duration::from_secs(5); while !predicate() { assert!(Instant::now()<end,"PTY condition timed out"); std::thread::sleep(Duration::from_millis(10)); } }
fn main() {
 let root=tempfile::tempdir().expect("tempdir"); let marker=root.path().join("marker");
 let s=Session::spawn(SpawnOptions { cwd:root.path().to_owned(), shell:Some("/run/current-system/sw/bin/bash".into()), args:vec!["--noprofile".into(),"--norc".into(),"-i".into()], columns:200, ..Default::default() }).expect("PTY");
 s.write(b"PS1=''; printf READY_%s done\r").unwrap(); until(||screen(&s).contains("READY_done"));
 let partial=format!("printf UNEXPECTED > {}; #PREFIX_RECEIVED",marker.display()); s.write(partial.as_bytes()).unwrap(); until(||screen(&s).contains("#PREFIX_RECEIVED")); assert!(!marker.exists());
 let result=futures_lite::future::block_on(s.change_directory(root.path())); println!("change_directory={result:?}");
 s.write(b"\x15printf BARRIER_%s done\r").unwrap(); until(||screen(&s).contains("BARRIER_done"));
 let executed=marker.exists(); println!("partial_command_executed={executed}");
 s.terminate(); futures_lite::future::block_on(s.wait_stopped());
 let expected=std::env::args().nth(1).as_deref()==Some("vulnerable"); assert_eq!(executed,expected,"unexpected partial-line execution outcome");
}

