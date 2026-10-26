//! Blocking desktop launch helpers. Call these from a background executor.

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail, ensure};

/// Open a regular document with the desktop's default application.
///
/// This is not an execution entry point: executable files and desktop launchers
/// require an explicit opening policy that the first application does not have.
/// Symbolic links are resolved before checking permissions and launcher suffixes.
///
/// # Errors
/// Returns an error for inaccessible/non-regular files, executable permissions,
/// `.desktop` launchers, or failures reported by the desktop opener.
pub fn open_file(path: &Path) -> Result<()> {
    let resolved = resolve(path)?;
    let metadata = fs::metadata(&resolved)
        .with_context(|| format!("无法读取文件信息：{}", resolved.display()))?;
    ensure!(metadata.is_file(), "不是普通文件：{}", path.display());
    ensure!(
        !is_desktop_launcher(path) && !is_desktop_launcher(&resolved),
        "不直接启动 .desktop 文件，请明确选择打开方式：{}",
        path.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o111 == 0,
            "文件具有可执行权限，请明确选择打开方式；当前入口不会执行它：{}",
            path.display()
        );
    }
    open::that(&resolved)
        .with_context(|| format!("无法使用默认应用打开文件：{}", resolved.display()))
}

/// Open an external terminal at an existing local directory.
///
/// `GDS_TERMINAL` is one executable path/name, never a shell command line. Its
/// process receives the directory as cwd and inherits the application environment.
/// Without that override, use `xdg-terminal-exec --dir=<directory>`; there is no
/// guessed emulator-specific argument or hard-coded shell. This waits for the
/// launcher to exit, which can be the terminal's lifetime for a non-daemonizing
/// emulator, so callers must keep it off the UI thread.
///
/// # Errors
/// Returns an error for an inaccessible/non-directory target, an empty override,
/// a missing terminal launcher, or a launcher failing to start or exiting with a
/// non-success status. External stderr/stdout are included in failure context.
pub fn open_terminal(directory: &Path) -> Result<()> {
    let directory = resolve(directory)?;
    let metadata = fs::metadata(&directory)
        .with_context(|| format!("无法读取目录信息：{}", directory.display()))?;
    ensure!(metadata.is_dir(), "不是目录：{}", directory.display());
    let (mut command, description) = match env::var_os("GDS_TERMINAL") {
        Some(program) => {
            ensure!(
                !program.is_empty(),
                "GDS_TERMINAL 为空；请设置为单个终端可执行文件路径，而不是 shell 命令"
            );
            let description = format!("GDS_TERMINAL 指定的终端 {:?}", program);
            (Command::new(program), description)
        }
        None => {
            let mut argument = OsString::from("--dir=");
            argument.push(directory.as_os_str());
            let mut command = Command::new("xdg-terminal-exec");
            command.arg(argument);
            (
                command,
                "xdg-terminal-exec（不可用时请安装它或配置 GDS_TERMINAL）".to_owned(),
            )
        }
    };
    command.current_dir(&directory).stdin(Stdio::null());
    let output = command
        .output()
        .with_context(|| format!("无法启动{description}，工作目录：{}", directory.display()))?;
    if !output.status.success() {
        bail!(
            "{description}退出失败（{}），工作目录：{}\n{}\n{}",
            output.status,
            directory.display(),
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        );
    }
    Ok(())
}

fn is_desktop_launcher(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("desktop"))
}

/// # Errors
/// Preserves the filesystem error for an unavailable target or broken link.
fn resolve(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!("无法访问位置：{}", path.display()))
}
