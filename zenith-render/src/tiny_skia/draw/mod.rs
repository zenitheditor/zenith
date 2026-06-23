//! Per-drawing-command rasterization, grouped by cohesion: fills/strokes
//! ([`shapes`] — each fill resolves its [`Paint`](zenith_scene::Paint), solid
//! or gradient), glyph runs ([`text`]), and image/SVG composites ([`image`]).
//! Each submodule's functions take the draw target, a
//! [`DrawCtx`](super::commands::DrawCtx), and the originating [`SceneCommand`];
//! the [`commands`](super::commands) dispatcher routes each variant to one of
//! them. Wiring only — no logic lives here.

pub(in crate::tiny_skia) mod image;
pub(in crate::tiny_skia) mod shapes;
pub(in crate::tiny_skia) mod text;
