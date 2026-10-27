//! Desktop launch helpers. Call these from a background executor.

#[cfg(target_os = "linux")]
pub mod clipboard;
#[cfg(target_os = "linux")]
mod portal;
pub mod watch;

#[cfg(test)]
mod tests;

use std::{
    env,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail, ensure};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenMode {
    DefaultApplication,
    ChooseApplication,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenOutcome {
    Opened,
    Cancelled,
}

/// Open a regular document, optionally asking the desktop to choose an application.
///
/// Executable permissions on recognised media do not turn them into programs.
/// Other executable files and desktop launchers require the application chooser.
/// Symbolic links are resolved before checking the target.
///
/// # Errors
/// Returns an error for inaccessible/non-regular files, executables opened without
/// a chooser, an unavailable chooser, or failures reported by the desktop opener.
pub async fn open_file(path: &Path, mode: OpenMode) -> Result<OpenOutcome> {
    let resolved = document_path(path, mode)?;
    if mode == OpenMode::ChooseApplication {
        #[cfg(target_os = "linux")]
        return portal::choose_application(&resolved).await;
        #[cfg(not(target_os = "linux"))]
        bail!("此平台尚不支持选择打开方式");
    }
    // xdg-open may live as long as the player. Observe immediate launch failures
    // without waiting for the document's application to exit.
    open::that_detached(&resolved)
        .with_context(|| format!("无法使用默认应用打开文件：{}", resolved.display()))?;
    Ok(OpenOutcome::Opened)
}

/// # Errors
/// Reports unavailable files and targets that require an explicit application.
fn document_path(path: &Path, mode: OpenMode) -> Result<PathBuf> {
    let resolved = resolve(path)?;
    let metadata = fs::metadata(&resolved)
        .with_context(|| format!("无法读取文件信息：{}", resolved.display()))?;
    ensure!(metadata.is_file(), "不是普通文件：{}", path.display());
    if mode == OpenMode::ChooseApplication {
        return Ok(resolved);
    }
    ensure!(
        !is_desktop_launcher(path) && !is_desktop_launcher(&resolved),
        "不直接启动 .desktop 文件，请明确选择打开方式：{}",
        path.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o111 == 0 || is_media(&resolved)?,
            "文件具有可执行权限，请明确选择打开方式；当前入口不会执行它：{}",
            path.display()
        );
    }
    Ok(resolved)
}

/// # Errors
/// Reports errors reading the file header.
fn is_media(path: &Path) -> Result<bool> {
    const HEADER_LIMIT: u64 = 8 * 1024;
    let mut header = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(HEADER_LIMIT).read_to_end(&mut header))
        .with_context(|| format!("无法读取文件内容：{}", path.display()))?;
    Ok(infer::get(&header).is_some_and(|kind| {
        matches!(
            kind.matcher_type(),
            infer::MatcherType::Video | infer::MatcherType::Audio | infer::MatcherType::Image
        )
    }))
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
