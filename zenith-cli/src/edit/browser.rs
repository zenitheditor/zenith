//! Open the editor page in the default browser without putting the token on
//! a command line.
//!
//! An opener command line (`xdg-open <url>`) is readable by every local user
//! through `/proc` or `ps`, and a cold-started browser keeps the URL in its
//! own argv. So the URL never goes there. [`LaunchFile`] writes a small HTML
//! file, readable by the owner only, that redirects to the URL, and the
//! opener gets the path of that file. The server removes the file after the
//! first authenticated request, and at exit.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zenith_session::adapter::{OsRng, Rng};

/// The redirect file. Dropping it removes the file.
#[derive(Debug)]
pub(crate) struct LaunchFile {
    path: PathBuf,
}

impl LaunchFile {
    /// Write the redirect file for `url` into the per-user runtime
    /// directory (`$XDG_RUNTIME_DIR`), else the temporary directory. The
    /// name is random, the file is new (never an existing one), and on Unix
    /// its mode is `0600`.
    ///
    /// # Errors
    ///
    /// The I/O or entropy error.
    pub(crate) fn create(url: &str) -> std::io::Result<LaunchFile> {
        Self::create_in(&launch_dir(), url)
    }

    fn create_in(dir: &Path, url: &str) -> std::io::Result<LaunchFile> {
        let mut bytes = [0u8; 16];
        OsRng
            .fill_bytes(&mut bytes)
            .map_err(|e| std::io::Error::other(e.message))?;
        let name: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let path = dir.join(format!("zenith-edit-{name}.html"));
        let mut file = create_private(&path)?;
        let launch = LaunchFile { path };
        file.write_all(redirect_page(url).as_bytes())?;
        file.sync_all()?;
        Ok(launch)
    }

    /// The file path.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for LaunchFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// `$XDG_RUNTIME_DIR` when it is an absolute directory, else the temporary
/// directory (per user on macOS and Windows).
fn launch_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_absolute() && d.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(unix)]
fn create_private(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// An HTML page that sends the browser to `url` at once. It names no
/// referrer, so the token never leaves in a `Referer` header.
fn redirect_page(url: &str) -> String {
    let url = html_escape(url);
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\">\
         <meta name=\"referrer\" content=\"no-referrer\">\
         <meta http-equiv=\"refresh\" content=\"0;url={url}\">\
         <title>zenith edit</title></head>\
         <body><p><a href=\"{url}\" rel=\"noreferrer\">Open zenith edit</a></p></body></html>\n"
    )
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Open `file` in the default browser: `open` on macOS, `rundll32
/// url.dll,FileProtocolHandler` on Windows, `xdg-open` elsewhere. The opener runs detached. A thread reaps
/// it.
///
/// # Errors
///
/// The spawn error, when the opener command is missing.
pub(crate) fn open(file: &Path) -> std::io::Result<()> {
    let mut command = opener(file);
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
fn opener(file: &Path) -> Command {
    let mut c = Command::new("open");
    c.arg(file);
    c
}

/// `rundll32 url.dll,FileProtocolHandler` opens the file with its handler.
/// No shell parses the path, so `&` or `^` in a user name changes nothing.
#[cfg(target_os = "windows")]
fn opener(file: &Path) -> Command {
    let mut c = Command::new("rundll32.exe");
    c.arg("url.dll,FileProtocolHandler").arg(file);
    c
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn opener(file: &Path) -> Command {
    let mut c = Command::new("xdg-open");
    c.arg(file);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "http://127.0.0.1:4242/#token=abc";

    #[test]
    fn the_redirect_file_is_private_random_and_removed_on_drop() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = LaunchFile::create_in(dir.path(), URL).expect("create");
        let b = LaunchFile::create_in(dir.path(), URL).expect("create");
        assert_ne!(a.path(), b.path(), "random names");
        let page = std::fs::read_to_string(a.path()).expect("read");
        assert!(
            page.contains("content=\"0;url=http://127.0.0.1:4242/#token=abc\""),
            "{page}"
        );
        assert!(page.contains("no-referrer"), "{page}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(a.path())
                .expect("meta")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let path = a.path().to_path_buf();
        drop(a);
        assert!(!path.exists(), "drop removes the file");
    }

    #[test]
    fn the_opener_never_sees_the_url() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = LaunchFile::create_in(dir.path(), URL).expect("create");
        let command = opener(file.path());
        let argv: Vec<String> = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(argv.iter().all(|a| !a.contains("token")), "{argv:?}");
        assert!(
            argv.iter().any(|a| a.ends_with(".html")),
            "the file path is passed: {argv:?}"
        );
    }

    #[test]
    fn markup_in_the_url_is_escaped() {
        let page = redirect_page("http://x/\"><script>&'");
        assert!(
            page.contains("&quot;&gt;&lt;script&gt;&amp;&#39;"),
            "{page}"
        );
        assert!(!page.contains("<script>"));
    }
}
