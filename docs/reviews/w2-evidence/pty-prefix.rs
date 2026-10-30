mod backend {
#[path = "/home/atlas/Github/4t145/gds/.agents/worktrees/4cecb57f-6267-4d96-8706-2d2b6717930c-w2/src/terminal/backend/shell.rs"] mod shell;
#[path = "/home/atlas/Github/4t145/gds/.agents/worktrees/4cecb57f-6267-4d96-8706-2d2b6717930c-w2/src/terminal/backend/unix.rs"] mod unix;
pub(crate) use unix::{Backend, Wake};
}
#[path = "/home/atlas/Github/4t145/gds/.agents/worktrees/4cecb57f-6267-4d96-8706-2d2b6717930c-w2/src/terminal/session.rs"] mod session;
#[path = "/home/atlas/Github/4t145/gds/.agents/worktrees/4cecb57f-6267-4d96-8706-2d2b6717930c-w2/src/terminal/directory.rs"] mod directory;
use std::{time::{Instant, Duration}, path::PathBuf};
fn main() {
 let root = std::env::temp_dir().join(format!("gds-prefix-{}", std::process::id()));
 std::fs::create_dir(&root).unwrap();
 let marker = root.join("unexpected-execution");
 let sh = std::env::args_os().nth(1).map(PathBuf::from).unwrap();
 let terminal = session::Session::spawn(session::SpawnOptions { cwd: root.clone(), shell: Some(sh), args: vec!["-i".into()], ..Default::default() }).unwrap();
 // Queue a harmless stand-in for a destructive command, with no Enter.
 let partial = format!("printf UNEXPECTED > {}; #", marker.display());
 terminal.write(partial.as_bytes()).unwrap();
 let deadline = Instant::now() + Duration::from_secs(4);
 // This wait is an experiment, not background-job polling: wait for echo to
 // establish that the PTY writer drained while readline retains the line.
 while !terminal.with_term(|t| t.selection_to_string().is_some()) && Instant::now() < deadline {
   std::thread::sleep(Duration::from_millis(20));
   if Instant::now() + Duration::from_millis(3700) > deadline { break; }
 }
 assert!(!marker.exists(), "no Enter was submitted");
 let result = futures_lite::future::block_on(terminal.change_directory(&root));
 println!("change_directory={result:?}");
 while !marker.exists() && Instant::now() < deadline { std::thread::sleep(Duration::from_millis(10)); }
 println!("partial_command_executed={}", marker.exists());
 terminal.terminate();
 futures_lite::future::block_on(terminal.wait_stopped());
 assert!(marker.exists(), "cd submission should expose the partial-line issue");
 std::fs::remove_dir_all(root).unwrap();
}
