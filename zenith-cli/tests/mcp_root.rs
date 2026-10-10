//! `zenith mcp --root`: the files a document reads are confined to the
//! root, as the paths a tool takes are. Unix only: the test links a file.
#![cfg(unix)]

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

fn req(id: u64, name: &str, args: Value) -> Value {
    json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": { "name": name, "arguments": args }
    })
}

/// Under `zenith mcp --root`, the files a document reads (here an image
/// asset reached through a link that points out of the root) are confined
/// too: the render refuses the read with a message that names the file, the
/// root, and the next action. Without a root the same document renders.
#[test]
fn a_documents_own_reads_are_confined_to_the_mcp_root() {
    let root = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    let data = tempfile::tempdir().expect("data");
    let png =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/assets/swatch.png"))
            .expect("swatch");
    std::fs::write(outside.path().join("secret.png"), &png).expect("write");
    std::fs::create_dir(root.path().join("assets")).expect("assets");
    std::os::unix::fs::symlink(
        outside.path().join("secret.png"),
        root.path().join("assets/swatch.png"),
    )
    .expect("symlink");
    let doc = root.path().join("d.zen");
    std::fs::write(
        &doc,
        r##"zenith version=1 {
  project id="proj.c" name="C"
  assets {
    asset id="a" kind="image" src="assets/swatch.png"
  }
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#f8fafc"
  }
  styles {}
  document id="doc.c" title="C" {
    page id="p" w=(px)100 h=(px)100 background=(token)"color.bg" {
      image id="i" asset="a" x=(px)0 y=(px)0 w=(px)50 h=(px)50 fit="stretch"
    }
  }
}
"##,
    )
    .expect("write doc");
    let run = |extra: &[&str]| -> Vec<Value> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zenith"))
            .arg("mcp")
            .args(extra)
            .env("ZENITH_DATA_DIR", data.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn zenith mcp");
        {
            let mut stdin = child.stdin.take().expect("stdin");
            for r in [
                req(
                    1,
                    "zenith_validate",
                    json!({ "doc": doc, "severity": "advisory" }),
                ),
                req(2, "zenith_render", json!({ "doc": doc, "format": "png" })),
            ] {
                writeln!(stdin, "{r}").expect("write request");
            }
        }
        let output = child.wait_with_output().expect("wait");
        String::from_utf8(output.stdout)
            .expect("utf8")
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).expect("json"))
            .collect()
    };
    let root_arg = root.path().to_string_lossy().into_owned();
    let confined = run(&["--root", &root_arg]);
    assert_eq!(confined.len(), 2, "{confined:?}");
    for reply in &confined {
        let text = reply.to_string();
        assert!(
            text.contains("asset.read_failed")
                && text.contains("outside the MCP root")
                && text.contains("swatch.png"),
            "{text}"
        );
    }
    let open = run(&[]);
    assert!(
        !open
            .iter()
            .any(|r| r.to_string().contains("outside the MCP root")),
        "{open:?}"
    );
}
