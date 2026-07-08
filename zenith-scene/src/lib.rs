//! Backend-neutral scene IR and scene compilation for Zenith.
//!
//! Owns the display-list primitives (`FillRect`, `DrawGlyphRun`, `PushClip`,
//! etc.), scene compilation from a validated AST into a z-ordered display list,
//! opacity/visibility/clip resolution, and exclusion of non-printing nodes.
//!
//! This crate is the stable contract boundary that every future backend —
//! GPU, PDF, SVG export — will consume.
//!
//! # Module layout
//!
//! - `ir`      — scene IR types (`Scene`, `SceneCommand`, `Color`, `SceneGlyph`).
//! - `color`   — sRGB hex parsing → `Color`.
//! - `compile` — `compile(&Document, &dyn FontProvider) -> CompileResult`.
//! - `compile_page_with_imports` — compile one page with a caller-owned in-memory import graph.
//! - `construction_overlay` — opt-in construction guide scene commands.
//! - `text_outline` — compiled glyph-run commands → editable compound paths.
//!
//! # Quick start
//!
//! ```rust
//! use zenith_scene::{Scene, SceneCommand, Color, Paint};
//!
//! let mut scene = Scene::new(640.0, 360.0);
//! scene.commands.push(SceneCommand::FillRect {
//!     x: 0.0, y: 0.0, w: 640.0, h: 360.0,
//!     paint: Paint::solid(Color::srgb(248, 250, 252, 255)),
//! });
//! let json = scene.to_json().expect("serializes");
//! assert!(json.contains("zenith-scene-v1"));
//! ```
//!
//! Compile a parsed document into a scene with
//! [`compile`](crate::compile::compile).

pub mod color;
pub mod compile;
pub mod construction_overlay;
pub mod ir;
pub mod text_outline;

// Curated flat re-exports.
pub use compile::{
    CompileResult, ImportGraph, ImportedDocument, compile, compile_page, compile_page_with_imports,
};
pub use construction_overlay::append_construction_overlay;
pub use ir::{
    BlendMode, Color, FillRule, FilterSpec, FitMode, GradientPaint, GradientStop, ImageClip,
    LineCap, LineJoin, MaskShape, MaskSpec, Paint, Rect, Scene, SceneCommand, SceneGlyph,
    ShadowSpec, SrcRect, StrokeAlign, SvgStyle,
};
pub use text_outline::{
    outline_glyph_run_command, outline_glyph_run_commands, outline_source_glyph_run_commands,
};
