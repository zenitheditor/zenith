//! The editor loops the benches measure, over the committed showcase deck
//! (`assets/showcase/zenith-presentation.zen`, 7 pages, 1600 × 900 px).
//!
//! - **typing**: the five engine calls that follow one keystroke
//!   (`buffer.set`, `doc.render` with a viewport, `doc.outline`,
//!   `doc.tokens`, `select.at_offset`), one call each, on page 2.
//! - **typing_batch**: the same five as one `commands.batch` call, as the
//!   page sends them, on page 2.
//! - **typing_heavy**: `typing_batch` on page 6, the deck's heaviest
//!   raster (six cards with two-layer blurred shadows).
//! - **drag**: one `gesture.preview` move step of a text node on page 2.
//! - **drag_snap**: the same step with snapping (`snap_distance` 6 page px).
//! - **drag_multi**: a snapped `gesture.preview` move of three nodes on
//!   page 2 as one selection.
//! - **buffer**: `buffer.set` alone.
//! - **render**: `doc.render` of page 2 as one whole-page viewport, scale 1.
//!
//! Each step starts from the same opened session ([`Fixture::start`]), so
//! every iteration does the same work.

use serde_json::{Value, json};
use zenith_editor::{MemProject, Outcome, Request, Session, Work};

/// The deck source.
pub const DECK: &str = include_str!("../../../assets/showcase/zenith-presentation.zen");
/// The deck's one asset.
const LOGO: &[u8] = include_bytes!("../../../assets/showcase/zenith-logo.svg");

/// The node the drag moves (page 2).
const DRAG_NODE: &str = "body-p2";
/// The selection the multi-node drag moves (page 2).
const DRAG_NODES: [&str; 3] = ["heading", "accent-rule-2", "body-p2"];

/// One keystroke: the session it starts from and the text after it.
pub struct Keystroke {
    /// The session after `doc.open` and the first render of the page.
    base: Session,
    /// The deck with one character typed.
    typed: String,
    /// The byte offset of the cursor after the keystroke.
    cursor: usize,
}

/// The opened deck and the keystrokes on page 2 and page 6.
pub struct Fixture {
    project: MemProject,
    /// A keystroke in `body-p2` on page 2.
    light: Keystroke,
    /// A keystroke in a card title on page 6.
    heavy: Keystroke,
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixture {
    /// Open the deck twice: on page 2 and on page 6, each rendered once.
    pub fn new() -> Fixture {
        let project = MemProject::new("zenith-presentation.zen").with_file("zenith-logo.svg", LOGO);
        let light = keystroke(&project, 2, "span \"Most design");
        let heavy = keystroke(&project, 6, "span \"Bind data");
        Fixture {
            project,
            light,
            heavy,
        }
    }

    /// The session scenario `name` starts from.
    pub fn start(&self, name: &str) -> Session {
        self.keystroke_of(name).base.clone()
    }

    fn keystroke_of(&self, name: &str) -> &Keystroke {
        if name == "typing_heavy" {
            &self.heavy
        } else {
            &self.light
        }
    }

    /// The typing loop as five calls. Returns the summed work.
    fn typing(&self, mut session: Session) -> Work {
        let mut work = Work::default();
        for request in requests(&self.light, session.version) {
            work += ok(&self.project, &mut session, request).work;
        }
        work
    }

    /// The typing loop as one `commands.batch` call (what the page sends).
    fn typing_batch(&self, key: &Keystroke, mut session: Session) -> Work {
        let steps = requests(key, session.version);
        let request = Request::new("commands.batch", json!({ "steps": steps }));
        let outcome = ok(&self.project, &mut session, request);
        let replies = outcome.result.as_ref().map(|v| v["steps"].clone());
        for step in replies
            .iter()
            .flat_map(|s| s.as_array().into_iter().flatten())
        {
            assert_eq!(step["ok"], true, "batch step failed: {step}");
        }
        outcome.work
    }

    /// One drag step: `gesture.preview` of a move with a viewport render.
    fn drag(&self, mut session: Session) -> Work {
        let request = Request::new(
            "gesture.preview",
            json!({
                "node": DRAG_NODE,
                "dx": 12,
                "dy": 7,
                "scale": 1,
                "viewport": viewport(),
            }),
        );
        ok(&self.project, &mut session, request).work
    }

    /// One snapped drag step: `drag` with `snap_distance` 6.
    fn drag_snap(&self, mut session: Session) -> Work {
        let request = Request::new(
            "gesture.preview",
            json!({
                "node": DRAG_NODE,
                "dx": 12,
                "dy": 7,
                "snap_distance": 6,
                "scale": 1,
                "viewport": viewport(),
            }),
        );
        ok(&self.project, &mut session, request).work
    }

    /// One snapped drag step of three nodes as one selection.
    fn drag_multi(&self, mut session: Session) -> Work {
        let request = Request::new(
            "gesture.preview",
            json!({
                "nodes": DRAG_NODES,
                "dx": 12,
                "dy": 7,
                "snap_distance": 6,
                "scale": 1,
                "viewport": viewport(),
            }),
        );
        ok(&self.project, &mut session, request).work
    }

    /// `buffer.set` of the keystroke alone.
    fn buffer(&self, mut session: Session) -> Work {
        let version = session.version;
        let request = Request::new("buffer.set", json!({ "text": self.light.typed })).at(version);
        ok(&self.project, &mut session, request).work
    }

    /// `doc.render` of the whole page as a viewport at scale 1.
    fn render(&self, mut session: Session) -> Work {
        ok(&self.project, &mut session, render_request()).work
    }

    /// Run scenario `name` (`typing`, `typing_batch`, `typing_heavy`,
    /// `drag`, `drag_snap`, `drag_multi`, `buffer`, or `render`) once from
    /// `session`
    /// ([`Fixture::start`]).
    pub fn run(&self, name: &str, session: Session) -> Work {
        match name {
            "typing" => self.typing(session),
            "typing_batch" => self.typing_batch(&self.light, session),
            "typing_heavy" => self.typing_batch(&self.heavy, session),
            "drag" => self.drag(session),
            "drag_snap" => self.drag_snap(session),
            "drag_multi" => self.drag_multi(session),
            "buffer" => self.buffer(session),
            "render" => self.render(session),
            other => panic!(
                "unknown scenario {other}; use typing, typing_batch, typing_heavy, drag, \
                 drag_snap, drag_multi, buffer, or render"
            ),
        }
    }
}

/// Open the deck on `page`, render it once, and type one character after
/// the first `anchor`.
fn keystroke(project: &MemProject, page: usize, anchor: &str) -> Keystroke {
    let at = DECK.find(anchor).expect("keystroke anchor") + anchor.len();
    let mut typed = DECK.to_owned();
    typed.insert(at, 'x');
    let mut base = Session::new("");
    ok(
        project,
        &mut base,
        Request::new("doc.open", json!({ "text": DECK })),
    );
    ok(
        project,
        &mut base,
        Request::new("view.set", json!({ "page": page })),
    );
    ok(project, &mut base, render_request());
    Keystroke {
        base,
        typed,
        cursor: at + 1,
    }
}

/// The five requests the page sends after one keystroke at `version`.
fn requests(key: &Keystroke, version: u64) -> [Request; 5] {
    [
        Request::new("buffer.set", json!({ "text": key.typed })).at(version),
        render_request(),
        Request::new("doc.outline", json!({})),
        Request::new("doc.tokens", json!({ "type": "color" })),
        Request::new("select.at_offset", json!({ "offset": key.cursor })),
    ]
}

/// The whole page as a viewport.
fn viewport() -> Value {
    json!({ "x": 0, "y": 0, "w": 1600, "h": 900 })
}

/// `doc.render` of the session page, whole page as a viewport, scale 1.
fn render_request() -> Request {
    Request::new("doc.render", json!({ "scale": 1, "viewport": viewport() }))
}

/// Run `request`; it must succeed. Keeps the next session.
fn ok(project: &MemProject, session: &mut Session, request: Request) -> Outcome {
    let outcome = project.execute(session.clone(), &request);
    if let Err(e) = &outcome.result {
        panic!("{} failed: {e:?}", request.command);
    }
    *session = outcome.session.clone();
    outcome
}
