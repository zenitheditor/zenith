//! Incremental structural scope tracking shared by export backends.

use std::{collections::BTreeMap, ops::Range};
use zenith_scene::SceneCommand;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Scope {
    Clip,
    Layer,
    Transform,
    Shadow,
    Blur,
    Filter,
    Mask,
}

enum Action {
    Open(Scope),
    Close(Scope),
    Draw,
}

fn action(command: &SceneCommand) -> Action {
    match command {
        SceneCommand::PushClip { .. } | SceneCommand::PushClipRoundedRect { .. } => {
            Action::Open(Scope::Clip)
        }
        SceneCommand::PopClip => Action::Close(Scope::Clip),
        SceneCommand::PushLayer { .. } => Action::Open(Scope::Layer),
        SceneCommand::PopLayer => Action::Close(Scope::Layer),
        SceneCommand::PushTransform { .. }
        | SceneCommand::PushScaleTranslate { .. }
        | SceneCommand::PushTransformMatrix { .. } => Action::Open(Scope::Transform),
        SceneCommand::PopTransform => Action::Close(Scope::Transform),
        SceneCommand::BeginShadow { .. } => Action::Open(Scope::Shadow),
        SceneCommand::EndShadow => Action::Close(Scope::Shadow),
        SceneCommand::BeginBlur { .. } => Action::Open(Scope::Blur),
        SceneCommand::EndBlur => Action::Close(Scope::Blur),
        SceneCommand::BeginFilter { .. } => Action::Open(Scope::Filter),
        SceneCommand::EndFilter => Action::Close(Scope::Filter),
        SceneCommand::BeginMask { .. } => Action::Open(Scope::Mask),
        SceneCommand::EndMask => Action::Close(Scope::Mask),
        SceneCommand::FillRect { .. }
        | SceneCommand::StrokeRect { .. }
        | SceneCommand::FillRoundedRect { .. }
        | SceneCommand::StrokeRoundedRect { .. }
        | SceneCommand::FillEllipse { .. }
        | SceneCommand::StrokeEllipse { .. }
        | SceneCommand::StrokeLine { .. }
        | SceneCommand::FillPolygon { .. }
        | SceneCommand::StrokePolyline { .. }
        | SceneCommand::FillPath { .. }
        | SceneCommand::StrokePath { .. }
        | SceneCommand::DrawImage { .. }
        | SceneCommand::DrawSvgAsset { .. }
        | SceneCommand::DrawGlyphRun { .. } => Action::Draw,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScopeError {
    DepthOverflow,
    UnmatchedClose { index: usize },
    Unfinished,
}

pub(crate) struct ScopeStep {
    pub(crate) crossed_now: bool,
    pub(crate) completed: Option<Range<usize>>,
}

#[derive(Default)]
pub(crate) struct ScopeTracker {
    stack: Vec<Scope>,
    counts: BTreeMap<Scope, usize>,
    crossed: bool,
    start: usize,
}

impl ScopeTracker {
    pub(crate) fn advance(
        &mut self,
        index: usize,
        command: &SceneCommand,
    ) -> Result<ScopeStep, ScopeError> {
        if self.counts.is_empty() {
            self.start = index;
        }
        let mut crossed_now = false;
        match action(command) {
            Action::Open(kind) => {
                let count = self.counts.entry(kind).or_default();
                *count = count.checked_add(1).ok_or(ScopeError::DepthOverflow)?;
                if !self.crossed {
                    self.stack.push(kind);
                }
            }
            Action::Close(kind) => {
                let count = self
                    .counts
                    .get_mut(&kind)
                    .ok_or(ScopeError::UnmatchedClose { index })?;
                *count = count
                    .checked_sub(1)
                    .ok_or(ScopeError::UnmatchedClose { index })?;
                if *count == 0 {
                    self.counts.remove(&kind);
                }
                if !self.crossed {
                    if self.stack.last() == Some(&kind) {
                        self.stack.pop();
                    } else {
                        self.crossed = true;
                        self.stack.clear();
                        crossed_now = true;
                    }
                }
            }
            Action::Draw => {}
        }
        let completed = if self.counts.is_empty() {
            self.crossed = false;
            Some(self.start..index.checked_add(1).ok_or(ScopeError::DepthOverflow)?)
        } else {
            None
        };
        Ok(ScopeStep {
            crossed_now,
            completed,
        })
    }

    pub(crate) fn finish(self) -> Result<(), ScopeError> {
        if self.counts.is_empty() {
            Ok(())
        } else {
            Err(ScopeError::Unfinished)
        }
    }
}
