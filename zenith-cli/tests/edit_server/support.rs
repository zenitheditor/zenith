//! Spawn `zenith edit` as a subprocess and talk to it with `std::net`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// A commented document with one rect at (20, 20) size 40x30.
pub const DOC: &str = r##"zenith version=1 {
  // Palette.
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#112233"
  }
  document id="doc.t" {
    // The only page.
    page id="p" w=(px)200 h=(px)120 {
      // The box an agent drags.
      rect id="box" x=(px)20 y=(px)20 w=(px)40 h=(px)30 fill=(token)"color.ink" // trailing
    }
  }
}
"##;

/// A running `zenith edit` and its temp directories.
pub struct Server {
    pub child: Child,
    pub dir: tempfile::TempDir,
    pub data: tempfile::TempDir,
    pub doc: PathBuf,
    pub port: u16,
    pub token: String,
    pub url: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start `zenith edit` on a copy of [`DOC`].
pub fn start() -> Server {
    start_with(DOC, &[])
}

/// Start `zenith edit` on `text` with extra args.
pub fn start_with(text: &str, extra: &[&str]) -> Server {
    let dir = tempfile::tempdir().expect("tempdir");
    let doc = dir.path().join("doc.zen");
    std::fs::write(&doc, text).expect("write doc");
    start_on(dir, doc, extra)
}

/// Start `zenith edit` on `doc` inside `dir`.
pub fn start_on(dir: tempfile::TempDir, doc: PathBuf, extra: &[&str]) -> Server {
    let data = tempfile::tempdir().expect("data dir");
    let mut child = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("edit")
        .arg(&doc)
        .args(["--no-open", "--json"])
        .args(extra)
        .env("ZENITH_DATA_DIR", data.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn zenith edit");
    let stdout = child.stdout.take().expect("stdout");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("read start line");
    let start: Value = serde_json::from_str(&line).unwrap_or_else(|e| {
        let mut err = String::new();
        if let Some(mut s) = child.stderr.take() {
            let _ = s.read_to_string(&mut err);
        }
        panic!("start line {line:?} is not JSON ({e}); stderr: {err}")
    });
    assert_eq!(start["schema"], "zenith-edit-v1");
    Server {
        port: u16::try_from(start["port"].as_u64().expect("port")).expect("u16"),
        token: start["token"].as_str().expect("token").to_owned(),
        url: start["url"].as_str().expect("url").to_owned(),
        child,
        dir,
        data,
        doc,
    }
}

/// One HTTP response.
#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub head: String,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|e| {
            panic!(
                "body is not JSON ({e}): {}",
                String::from_utf8_lossy(&self.body)
            )
        })
    }

    pub fn header(&self, name: &str) -> Option<String> {
        self.head.lines().find_map(|l| {
            let (n, v) = l.split_once(':')?;
            n.eq_ignore_ascii_case(name).then(|| v.trim().to_owned())
        })
    }
}

/// Send `raw` (a full request) and read the whole response.
pub fn raw(port: u16, raw: &[u8]) -> Reply {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(60)))
        .expect("timeout");
    s.write_all(raw).expect("write");
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    let split = out
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no head in {:?}", String::from_utf8_lossy(&out)));
    let head = String::from_utf8_lossy(&out[..split]).into_owned();
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("status");
    Reply {
        status,
        head,
        body: out[split + 4..].to_vec(),
    }
}

/// Send `raw` and read only the status code (the stream may stay open).
pub fn status_only(port: u16, raw: &str) -> u16 {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    s.write_all(raw.as_bytes()).expect("write");
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).expect("status line");
    line.split(' ')
        .nth(1)
        .and_then(|c| c.parse().ok())
        .expect("status")
}

impl Server {
    /// The `Host` header value.
    pub fn host(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// A request with the bearer token and `extra` headers.
    pub fn request(
        &self,
        method: &str,
        path: &str,
        extra: &[(&str, &str)],
        body: Option<&str>,
    ) -> Reply {
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n",
            self.host(),
            self.token
        );
        for (n, v) in extra {
            head.push_str(&format!("{n}: {v}\r\n"));
        }
        if let Some(b) = body {
            head.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                b.len()
            ));
        }
        head.push_str("\r\n");
        let mut bytes = head.into_bytes();
        if let Some(b) = body {
            bytes.extend_from_slice(b.as_bytes());
        }
        raw(self.port, &bytes)
    }

    /// `POST /api/cmd` as client `client`. Returns the envelope.
    pub fn cmd_as(&self, client: &str, body: Value) -> Value {
        let r = self.request(
            "POST",
            "/api/cmd",
            &[("X-Zenith-Client", client)],
            Some(&body.to_string()),
        );
        assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
        r.json()
    }

    /// `POST /api/cmd`. Returns the envelope.
    pub fn cmd(&self, body: Value) -> Value {
        self.cmd_as("test", body)
    }

    /// `POST /api/cmd` that must succeed. Returns the result.
    pub fn ok(&self, body: Value) -> Value {
        let env = self.cmd(body);
        assert_eq!(env["ok"], true, "{env}");
        env["result"].clone()
    }

    /// `GET /api/state?text=1`.
    pub fn state(&self) -> Value {
        let r = self.request("GET", "/api/state?text=1", &[], None);
        assert_eq!(r.status, 200);
        r.json()
    }

    /// The session version.
    pub fn version(&self) -> u64 {
        self.state()["version"].as_u64().expect("version")
    }

    /// Open an event stream as `client`.
    pub fn events(&self) -> Events {
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        let head = format!(
            "GET /api/events HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\r\n",
            self.host(),
            self.token
        );
        s.write_all(head.as_bytes()).expect("write");
        let mut events = Events {
            reader: BufReader::new(s),
        };
        let status = events.line(Duration::from_secs(10)).expect("status line");
        assert!(status.starts_with("HTTP/1.1 200"), "{status}");
        while events
            .line(Duration::from_secs(10))
            .is_some_and(|l| !l.is_empty())
        {}
        let (name, _) = events.next(Duration::from_secs(10)).expect("state");
        assert_eq!(name, "state");
        events
    }

    /// Send `signal` (`INT`, `TERM`, or `KILL`) to the server process.
    pub fn signal(&self, signal: &str) {
        let pid = self.child.id().to_string();
        let status = Command::new("kill")
            .args(["-s", signal, &pid])
            .status()
            .expect("run kill");
        assert!(status.success(), "kill -s {signal} {pid}: {status}");
    }

    /// `true` while the server process runs.
    #[cfg(unix)]
    pub fn alive(&mut self) -> bool {
        self.child.try_wait().expect("try_wait").is_none()
    }

    /// Send `signal` (`KILL` kills on every platform), wait for the exit,
    /// and return the exit status and everything the server wrote to
    /// stderr.
    pub fn stop_with_signal_and_collect(&mut self, signal: &str) -> (ExitStatus, String) {
        if signal == "KILL" {
            self.child.kill().expect("kill");
        } else {
            self.signal(signal);
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("try_wait") {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "server did not exit after {signal}"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        let mut stderr = String::new();
        if let Some(mut s) = self.child.stderr.take() {
            let _ = s.read_to_string(&mut stderr);
        }
        (status, stderr)
    }

    /// Stop the server through the API and wait for exit 0.
    pub fn shutdown(mut self, force: bool) {
        let r = self.request(
            "POST",
            "/api/shutdown",
            &[],
            Some(&json!({ "force": force }).to_string()),
        );
        assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.child.try_wait().expect("try_wait") {
                assert!(status.success(), "{status}");
                return;
            }
            assert!(Instant::now() < deadline, "server did not stop");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// A Server-Sent Events reader.
pub struct Events {
    reader: BufReader<TcpStream>,
}

impl Events {
    fn line(&mut self, wait: Duration) -> Option<String> {
        self.reader
            .get_ref()
            .set_read_timeout(Some(wait))
            .expect("timeout");
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim_end_matches(['\r', '\n']).to_owned()),
        }
    }

    /// The next event within `wait`. Ping comments are skipped.
    pub fn next(&mut self, wait: Duration) -> Option<(String, Value)> {
        let deadline = Instant::now() + wait;
        let mut name = String::new();
        let mut data = String::new();
        loop {
            let left = deadline.checked_duration_since(Instant::now())?;
            let line = self.line(left.max(Duration::from_millis(1)))?;
            if line.is_empty() {
                if !name.is_empty() {
                    let value = serde_json::from_str(&data).expect("event data");
                    return Some((name, value));
                }
                continue;
            }
            if let Some(v) = line.strip_prefix("event: ") {
                name = v.to_owned();
            } else if let Some(v) = line.strip_prefix("data: ") {
                data = v.to_owned();
            }
        }
    }

    /// The next event named `name` within `wait`, skipping others.
    pub fn wait_for(&mut self, name: &str, wait: Duration) -> Value {
        let deadline = Instant::now() + wait;
        loop {
            let left = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| panic!("no '{name}' event in time"));
            match self.next(left) {
                Some((n, v)) if n == name => return v,
                Some(_) => {}
                None => panic!("no '{name}' event in time"),
            }
        }
    }
}

/// Apply a wire delta (`from`/`to` in UTF-16 units) to `text`.
pub fn apply_delta(text: &str, delta: &Value) -> String {
    let from = usize::try_from(delta["from"].as_u64().expect("from")).expect("usize");
    let to = usize::try_from(delta["to"].as_u64().expect("to")).expect("usize");
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut out: Vec<u16> = units[..from].to_vec();
    out.extend(delta["insert"].as_str().expect("insert").encode_utf16());
    out.extend_from_slice(&units[to..]);
    String::from_utf16(&out).expect("utf16")
}

/// Read `path` as text.
pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read")
}
