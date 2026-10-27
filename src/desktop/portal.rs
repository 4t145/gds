use std::{fs::File, path::Path};

use anyhow::{Context, Result};
use ashpd::desktop::{ResponseError, open_uri::OpenFileRequest};

use super::OpenOutcome;

/// # Errors
/// Reports unavailable files, portal failures and application launch failures.
pub(super) async fn choose_application(path: &Path) -> Result<OpenOutcome> {
    let file = File::open(path).with_context(|| format!("无法读取文件：{}", path.display()))?;
    // OpenFile passes an fd, preserving non-UTF-8 names and asking the desktop
    // to open a document instead of executing the selected file.
    // https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.OpenURI.html
    let response = OpenFileRequest::default()
        .ask(true)
        .send_file(&file)
        .await
        .and_then(|request| request.response());
    match response {
        Ok(()) => Ok(OpenOutcome::Opened),
        Err(ashpd::Error::Response(ResponseError::Cancelled)) => Ok(OpenOutcome::Cancelled),
        Err(error) => Err(error).with_context(|| {
            format!(
                "无法打开文件：{}；请检查系统 OpenURI / AppChooser Portal 和默认应用",
                path.display()
            )
        }),
    }
}
