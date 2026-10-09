//! Open a URL in the default browser with the OS opener command.

use std::process::{Command, Stdio};

/// Open `url` in the default browser: `open` on macOS, `cmd /C start` on
/// Windows, `xdg-open` elsewhere. The opener runs detached. A thread
/// reaps it.
///
/// # Errors
///
/// The spawn error, when the opener command is missing.
pub(crate) fn open(url: &str) -> std::io::Result<()> {
    let mut command = opener(url);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(target_os = "macos")]
fn opener(url: &str) -> Command {
    let mut c = Command::new("open");
    c.arg(url);
    c
}

#[cfg(target_os = "windows")]
fn opener(url: &str) -> Command {
    let mut c = Command::new("cmd");
    c.args(["/C", "start", ""]).arg(url);
    c
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn opener(url: &str) -> Command {
    let mut c = Command::new("xdg-open");
    c.arg(url);
    c
}
