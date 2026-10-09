//! Shared helpers: a session driver over a [`MemProject`], example loading,
//! and float checks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use zenith_editor::{EditorError, MemProject, Outcome, Request, Session, commands};

/// The workspace `examples/` directory.
pub fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples")
}

/// Every file under `examples/` except the top-level PNG renders, keyed by
/// its path relative to `examples/`.
pub fn example_files() -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .expect("read examples dir")
            .map(|e| e.expect("dir entry").path())
            .collect();
        entries.sort();
        for path in entries {
            let rel = path
                .strip_prefix(root)
                .expect("under root")
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                walk(&path, root, out);
            } else if !(path.parent() == Some(root) && rel.ends_with(".png")) {
                out.insert(rel, std::fs::read(&path).expect("read file"));
            }
        }
    }
    let root = examples_dir();
    let mut out = BTreeMap::new();
    walk(&root, &root, &mut out);
    out
}

/// The `.zen` example names, sorted.
pub fn example_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(examples_dir())
        .expect("read examples dir")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n.ends_with(".zen"))
        .collect();
    names.sort();
    names
}

/// A session driven over one project.
pub struct Driver {
    pub project: MemProject,
    pub session: Session,
}

impl Driver {
    /// A driver over an empty project with `text` opened.
    pub fn open(text: &str) -> Driver {
        Driver::open_in(MemProject::new("document.zen"), text)
    }

    /// A driver over `project` with `text` opened.
    pub fn open_in(project: MemProject, text: &str) -> Driver {
        let mut d = Driver {
            project,
            session: Session::new(""),
        };
        d.ok("doc.open", json!({ "text": text }));
        d
    }

    /// A driver over example `name` with every example file present.
    pub fn example(name: &str) -> Driver {
        let text = String::from_utf8(
            example_files()
                .remove(name)
                .unwrap_or_else(|| panic!("example {name}")),
        )
        .expect("utf8");
        let project = MemProject::new(name).with_files(example_files());
        Driver::open_in(project, &text)
    }

    /// Send `command` with `params`, at the current version when the
    /// command needs one. Keeps the next session.
    pub fn outcome(&mut self, command: &str, params: Value) -> Outcome {
        let needs = commands()
            .iter()
            .any(|c| c.id() == command && c.needs_version());
        let mut request = Request::new(command, params);
        if needs {
            request = request.at(self.session.version);
        }
        self.send(&request)
    }

    /// Send `request` as is and keep the next session.
    pub fn send(&mut self, request: &Request) -> Outcome {
        let outcome = self.project.execute(self.session.clone(), request);
        self.session = outcome.session.clone();
        outcome
    }

    /// `command` must succeed; its reply.
    pub fn ok(&mut self, command: &str, params: Value) -> Value {
        match self.outcome(command, params.clone()).result {
            Ok(v) => v,
            Err(e) => panic!("{command} {params}: {e:?}"),
        }
    }

    /// `command` must fail; its error. The session must be unchanged.
    pub fn err(&mut self, command: &str, params: Value) -> EditorError {
        let before = self.session.clone();
        let outcome = self.outcome(command, params.clone());
        assert_eq!(outcome.session, before, "{command} changed the session");
        match outcome.result {
            Ok(v) => panic!("{command} {params} succeeded: {v}"),
            Err(e) => e,
        }
    }

    /// The page corners of node `id`, from `node.inspect`.
    pub fn corners(&mut self, id: &str) -> [(f64, f64); 4] {
        let v = self.ok("node.inspect", json!({ "id": id }));
        corners_of(&v["box"]["corners"])
    }

    /// The handle `handle` of node `id`, from `node.handles`.
    pub fn handle(&mut self, id: &str, handle: &str) -> Value {
        let v = self.ok("node.handles", json!({ "id": id }));
        v["handles"]
            .as_array()
            .expect("handles")
            .iter()
            .find(|h| h["id"] == handle)
            .cloned()
            .unwrap_or_else(|| panic!("no handle {handle} on {id}: {v}"))
    }
}

/// Four `[x, y]` pairs.
pub fn corners_of(v: &Value) -> [(f64, f64); 4] {
    let pts: Vec<(f64, f64)> = v
        .as_array()
        .unwrap_or_else(|| panic!("corners: {v}"))
        .iter()
        .map(|p| (p[0].as_f64().expect("x"), p[1].as_f64().expect("y")))
        .collect();
    [pts[0], pts[1], pts[2], pts[3]]
}

/// `true` when the two points are within `eps`.
pub fn near(a: (f64, f64), b: (f64, f64), eps: f64) -> bool {
    (a.0 - b.0).abs() <= eps && (a.1 - b.1).abs() <= eps
}

/// Assert every corner of `after` is `before` moved by `(dx, dy)`.
pub fn assert_moved(before: [(f64, f64); 4], after: [(f64, f64); 4], dx: f64, dy: f64, what: &str) {
    for (b, a) in before.iter().zip(after.iter()) {
        assert!(
            near((b.0 + dx, b.1 + dy), *a, 1e-6),
            "{what}: {b:?} + ({dx}, {dy}) != {a:?}"
        );
    }
}

/// The codes of `e`'s error diagnostics.
pub fn codes(e: &EditorError) -> Vec<String> {
    e.diagnostics
        .iter()
        .filter(|d| d.severity == "error")
        .map(|d| d.code.clone())
        .collect()
}

/// The offer ids of `e`.
pub fn offer_ids(e: &EditorError) -> Vec<String> {
    e.offers.iter().map(|o| o.id.clone()).collect()
}

/// A minimal document with `body` inside one 400 × 300 page and a few
/// tokens.
pub fn doc(body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.t" name="T"
  tokens format="zenith-token-v1" {{
    token id="color.ink" type="color" value="#203040"
    token id="color.bg" type="color" value="#f0f0f0"
    token id="size.x" type="dimension" value=(px)30
    token id="size.w" type="dimension" value=(px)80
  }}
  styles {{}}
  document id="doc.t" title="T" {{
    page id="pg" w=(px)400 h=(px)300 {{
{body}
    }}
  }}
}}
"##
    )
}
