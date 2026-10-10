//! The transform and clips open at any point of the command streams one
//! recorder reads, kept as the streams grow.
//!
//! A box record needs the state the commands before the node's first
//! command leave open. A fresh fold of that prefix per node costs O(n²) on
//! a page of n siblings. [`Tracks`] keeps forward-moving cursors instead, so
//! each command folds once per nesting level.
//!
//! A stream is the `Vec` a node compiles into, named by the address of the
//! `Vec` ([`StreamId`]). The cursors rest on these facts of the compile:
//!
//! - Every recorded node opens a [`Frame`] before its first command and
//!   closes it before its record.
//! - A node only appends to its stream before its record. It edits nothing
//!   before its own first command.
//! - A container compiles its children into its own stream or into one
//!   scratch `Vec` it creates. The scratch `Vec` lives until the
//!   container's last child is recorded. An effect or mask wrapper and a
//!   pattern record their scratch children in a nested recorder: its own
//!   tracks, its base the state open where the scratch `Vec` is spliced.
//! - A page-level rewrite of the stream (connector line jumps) calls
//!   [`Tracks::rewrote`].
//!
//! A cursor for a frame's own stream starts at the frame's first command
//! and lives in the frame. A cursor for any other stream lives in the
//! innermost open frame, or at the root when no frame is open, so it is
//! dropped before its `Vec` can be dropped and the address reused. A cursor
//! asked for a point before its own refolds from the base. So every answer
//! equals the fold of the whole prefix from the base.

use crate::ir::SceneCommand;

use super::clip::{Open, Tracker};

/// The identity of one command stream: the address of its `Vec`. Two live
/// streams never share it. It is never dereferenced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::compile) struct StreamId(usize);

impl StreamId {
    /// The identity of `commands`.
    pub(in crate::compile) fn of(commands: &mut Vec<SceneCommand>) -> Self {
        Self(std::ptr::from_mut(commands).addr())
    }
}

/// The fold of one stream up to `pos`.
#[derive(Clone, Debug)]
struct Cursor {
    stream: StreamId,
    pos: usize,
    state: Tracker,
}

impl Cursor {
    /// Move to `at` in `commands`. A point before the cursor, or past the
    /// end, refolds from `base`.
    fn seek(&mut self, base: &Open, commands: &[SceneCommand], at: usize) {
        if at < self.pos || self.pos > commands.len() {
            self.pos = 0;
            self.state = Tracker::new(base);
        }
        let end = at.min(commands.len());
        for cmd in commands.get(self.pos..end).unwrap_or_default() {
            self.state.step(cmd);
        }
        self.pos = end;
    }
}

/// One compiling node: its stream, its first command, and the cursors it
/// holds.
#[derive(Debug)]
struct Frame {
    stream: StreamId,
    start: usize,
    cursors: Vec<Cursor>,
}

impl Frame {
    fn holds(&self, stream: StreamId) -> bool {
        self.cursors.iter().any(|c| c.stream == stream)
    }
}

/// The open frames and root cursors of one recorder.
#[derive(Debug, Default)]
pub(super) struct Tracks {
    frames: Vec<Frame>,
    root: Vec<Cursor>,
}

impl Tracks {
    /// A node starts compiling at `start` in `stream`.
    pub(super) fn enter(&mut self, stream: StreamId, start: usize) {
        self.frames.push(Frame {
            stream,
            start,
            cursors: Vec::new(),
        });
    }

    /// The innermost compiling node is done.
    pub(super) fn leave(&mut self) {
        self.frames.pop();
    }

    /// A pass rewrote a stream: drop every cursor.
    pub(super) fn rewrote(&mut self) {
        self.root.clear();
        for frame in &mut self.frames {
            frame.cursors.clear();
        }
    }

    /// The state open after `commands[..at]`, which run under `base`.
    /// `at` past the end opens nothing past `base`.
    pub(super) fn open_at(
        &mut self,
        base: &Open,
        stream: StreamId,
        commands: &[SceneCommand],
        at: usize,
    ) -> Open {
        if at > commands.len() {
            return base.clone();
        }
        match cursor(&mut self.frames, &mut self.root, base, stream, commands, at) {
            Some(c) => c.state.open(),
            None => fold(base, commands, at).open(),
        }
    }
}

/// The fold of `commands[..at]` from `base`, without a cursor.
fn fold(base: &Open, commands: &[SceneCommand], at: usize) -> Tracker {
    let mut state = Tracker::new(base);
    for cmd in commands.get(..at).unwrap_or_default() {
        state.step(cmd);
    }
    state
}

/// The cursor of `stream`, moved to `at`. It comes from the innermost frame
/// that compiles into `stream` or holds a cursor for it, else from the
/// root, else it is created in the innermost frame (the root when none is
/// open). A frame's first cursor for its own stream starts from the state
/// at the frame's first command, asked of the frames outside it.
fn cursor<'t>(
    frames: &'t mut [Frame],
    root: &'t mut Vec<Cursor>,
    base: &Open,
    stream: StreamId,
    commands: &[SceneCommand],
    at: usize,
) -> Option<&'t mut Cursor> {
    let owner = frames
        .iter()
        .rposition(|f| f.stream == stream || f.holds(stream));
    let cursors: &'t mut Vec<Cursor> = match owner {
        Some(depth) => {
            let (outer, rest) = frames.split_at_mut(depth);
            let frame = rest.first_mut()?;
            if !frame.holds(stream) {
                let state = match cursor(outer, root, base, stream, commands, frame.start) {
                    Some(c) => c.state.clone(),
                    None => fold(base, commands, frame.start),
                };
                frame.cursors.push(Cursor {
                    stream,
                    pos: frame.start,
                    state,
                });
            }
            &mut frame.cursors
        }
        None if root.iter().any(|c| c.stream == stream) => root,
        None => match frames.last_mut() {
            Some(frame) => &mut frame.cursors,
            None => root,
        },
    };
    let index = match cursors.iter().position(|c| c.stream == stream) {
        Some(index) => index,
        None => {
            cursors.push(Cursor {
                stream,
                pos: 0,
                state: Tracker::new(base),
            });
            cursors.len() - 1
        }
    };
    let c = cursors.get_mut(index)?;
    c.seek(base, commands, at);
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Color, Paint};

    /// A small deterministic generator (64-bit LCG).
    struct Gen(u64);

    impl Gen {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }

        fn coord(&mut self) -> f64 {
            (self.below(4000) as f64) / 8.0 - 100.0
        }
    }

    /// One random push, and the pop that closes it.
    fn push(g: &mut Gen) -> (SceneCommand, SceneCommand) {
        match g.below(5) {
            0 => (
                SceneCommand::PushClip {
                    x: g.coord(),
                    y: g.coord(),
                    w: g.coord().abs(),
                    h: g.coord().abs(),
                },
                SceneCommand::PopClip,
            ),
            1 => (
                SceneCommand::PushClipRoundedRect {
                    x: g.coord(),
                    y: g.coord(),
                    w: g.coord().abs(),
                    h: g.coord().abs(),
                    radius: (g.below(20) as f64) / 2.0,
                },
                SceneCommand::PopClip,
            ),
            2 => (
                SceneCommand::PushTransform {
                    angle_deg: g.coord(),
                    cx: g.coord(),
                    cy: g.coord(),
                },
                SceneCommand::PopTransform,
            ),
            3 => (
                SceneCommand::PushScaleTranslate {
                    sx: 0.5 + (g.below(8) as f64) / 4.0,
                    sy: 0.5 + (g.below(8) as f64) / 4.0,
                    tx: g.coord(),
                    ty: g.coord(),
                },
                SceneCommand::PopTransform,
            ),
            _ => (
                SceneCommand::PushTransformMatrix {
                    a: 1.0,
                    b: (g.below(5) as f64) / 10.0,
                    c: -(g.below(5) as f64) / 10.0,
                    d: 1.0,
                    e: g.coord(),
                    f: g.coord(),
                },
                SceneCommand::PopTransform,
            ),
        }
    }

    fn ink(g: &mut Gen) -> SceneCommand {
        SceneCommand::FillRect {
            x: g.coord(),
            y: g.coord(),
            w: 4.0,
            h: 4.0,
            paint: Paint::solid(Color::srgb(0, 0, 0, 255)),
        }
    }

    /// Every state the tracks reported: the point asked for, and the state.
    type Answers = Vec<(usize, String)>;

    /// The state the tracks report equals the reference fold of the prefix.
    fn check(
        tracks: &mut Tracks,
        base: &Open,
        commands: &mut Vec<SceneCommand>,
        at: usize,
        answers: &mut Answers,
    ) {
        let stream = StreamId::of(commands);
        let got = format!("{:?}", tracks.open_at(base, stream, commands, at));
        let want = base.after(commands.get(..at).unwrap_or_default());
        assert_eq!(got, format!("{want:?}"), "at {at}");
        answers.push((at, got));
    }

    /// Compile the children of an effect or mask wrapper the way
    /// `emit_wrapped_container` drives a nested recorder: a scratch stream
    /// under fresh tracks, its base the state open at the splice point.
    /// Every answer the nested tracks gave equals the fold of the final
    /// stream from the outer base, at the spliced point.
    fn wrapped(
        g: &mut Gen,
        tracks: &mut Tracks,
        base: &Open,
        commands: &mut Vec<SceneCommand>,
        depth: u32,
        answers: &mut Answers,
    ) {
        let splice = commands.len();
        let stream = StreamId::of(commands);
        let inner_base = tracks.open_at(base, stream, commands, splice);
        let mut inner_tracks = Tracks::default();
        let mut scratch = Vec::new();
        let mut inner = Answers::new();
        for _ in 0..g.below(5) {
            node(
                g,
                &mut inner_tracks,
                &inner_base,
                &mut scratch,
                (depth, true),
                &mut inner,
            );
        }
        // An effect alone leads with one command. An effect with a mask
        // draws a sharp copy first, so records point at that copy.
        let lead = if g.below(2) == 0 {
            commands.push(SceneCommand::BeginBlur { radius: 2.0 });
            commands.extend(scratch);
            commands.push(SceneCommand::EndBlur);
            1
        } else {
            commands.extend(scratch.iter().cloned());
            commands.push(SceneCommand::BeginBlur { radius: 2.0 });
            commands.extend(scratch);
            commands.push(SceneCommand::EndBlur);
            0
        };
        for (at, got) in inner {
            let at = splice + lead + at;
            let want = base.after(commands.get(..at).unwrap_or_default());
            assert_eq!(got, format!("{want:?}"), "spliced at {at}");
            answers.push((at, got));
        }
    }

    /// Compile one generated node into `commands` the way `compile_node`
    /// drives a recorder: open a frame, emit, close the frame, record.
    fn node(
        g: &mut Gen,
        tracks: &mut Tracks,
        base: &Open,
        commands: &mut Vec<SceneCommand>,
        (depth, sealed): (u32, bool),
        checks: &mut Answers,
    ) {
        let start = commands.len();
        tracks.enter(StreamId::of(commands), start);
        let mut closes = Vec::new();
        for _ in 0..g.below(3) {
            let (open, close) = push(g);
            commands.push(open);
            closes.push(close);
        }
        // An unbalanced pop: it closes a push from outside the node, or
        // nothing when the base opened it. Under a wrapper (`sealed`) a pop
        // never closes a push from before the wrapper: the backends reject
        // a close that crosses an effect or mask scope.
        if g.below(6) == 0 && !sealed {
            commands.push(if g.below(2) == 0 {
                SceneCommand::PopClip
            } else {
                SceneCommand::PopTransform
            });
        }
        // A mid-compile query of the own stream, as `pattern` asks.
        if g.below(4) == 0 {
            let at = commands.len();
            check(tracks, base, commands, at, checks);
        }
        if depth > 0 && g.below(4) != 0 {
            let children = g.below(5);
            if g.below(3) == 0 {
                // An effect wrapper: the children compile into a scratch
                // stream under a nested recorder, then splice in.
                wrapped(g, tracks, base, commands, depth - 1, checks);
            } else {
                for _ in 0..children {
                    node(g, tracks, base, commands, (depth - 1, sealed), checks);
                }
            }
        } else {
            commands.push(ink(g));
        }
        while let Some(close) = closes.pop() {
            commands.push(close);
        }
        tracks.leave();
        // Nodes without an id record nothing.
        if g.below(5) != 0 {
            check(tracks, base, commands, start, checks);
        }
    }

    /// A scratch stream's cursor dies with the frame open while it was made:
    /// a later scratch `Vec` at the same address starts a fresh fold.
    #[test]
    fn scratch_cursors_close_with_their_frame() {
        let base = Open::default();
        let mut tracks = Tracks::default();
        let mut page = vec![ink(&mut Gen(1))];
        let mut scratch = Vec::new();
        let mut checks = Answers::new();
        for clip in [true, false] {
            // A container opens a frame on the page; its children compile
            // into the same scratch `Vec`, cleared in between.
            tracks.enter(StreamId::of(&mut page), page.len());
            // The second stream reaches past the first one's end.
            scratch.clear();
            scratch.push(if clip {
                SceneCommand::PushClip {
                    x: 0.0,
                    y: 0.0,
                    w: 10.0,
                    h: 10.0,
                }
            } else {
                ink(&mut Gen(2))
            });
            scratch.push(ink(&mut Gen(3)));
            if !clip {
                scratch.push(ink(&mut Gen(4)));
            }
            let at = scratch.len();
            check(&mut tracks, &base, &mut scratch, at, &mut checks);
            tracks.leave();
        }
        assert_eq!(checks.len(), 2);
    }

    /// Generated nested clip and transform streams: every point a record
    /// asks for matches the full fold of its prefix.
    #[test]
    fn tracks_match_the_reference_fold() {
        let mut checks = Answers::new();
        for seed in 0..40_u64 {
            let mut g = Gen(seed);
            // A nested recorder's base carries clips and a transform; a
            // page recorder's is empty.
            let base = if seed % 2 == 0 {
                Open::default()
            } else {
                let (a, _) = push(&mut g);
                let lead = [
                    SceneCommand::PushClip {
                        x: 0.0,
                        y: 0.0,
                        w: 500.0,
                        h: 400.0,
                    },
                    SceneCommand::PushScaleTranslate {
                        sx: 2.0,
                        sy: 2.0,
                        tx: 5.0,
                        ty: 5.0,
                    },
                    a,
                ];
                Open::default().after(&lead)
            };
            let mut tracks = Tracks::default();
            let mut page = Vec::new();
            for _ in 0..(10 + g.below(30)) {
                node(
                    &mut g,
                    &mut tracks,
                    &base,
                    &mut page,
                    (4, false),
                    &mut checks,
                );
            }
            // A rewrite inserts a clip mid-stream (line jumps change the
            // stream in place), then root-level records follow.
            let at = page.len() / 2;
            page.insert(
                at,
                SceneCommand::PushClip {
                    x: 1.0,
                    y: 2.0,
                    w: 30.0,
                    h: 40.0,
                },
            );
            tracks.rewrote();
            for _ in 0..5 {
                let start = page.len();
                page.push(ink(&mut g));
                check(&mut tracks, &base, &mut page, start, &mut checks);
            }
            // A point before the cursor and a point past the end.
            check(&mut tracks, &base, &mut page, 1, &mut checks);
            let past = page.len() + 3;
            check(&mut tracks, &base, &mut page, past, &mut checks);
        }
        assert!(checks.len() > 2000, "only {} checks ran", checks.len());
    }
}
