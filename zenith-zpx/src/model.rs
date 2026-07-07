use zenith_core::{BlendMode, Color, GradientStop};

use crate::error::ZpxError;

#[derive(Debug, Clone, PartialEq)]
pub struct ZpxDoc {
    pub canvas: Canvas,
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Canvas {
    pub width_px: u32,
    pub height_px: u32,
    pub color_space: ColorSpace,
    pub alpha_mode: AlphaMode,
}

impl Canvas {
    pub const fn new(width_px: u32, height_px: u32) -> Self {
        Self {
            width_px,
            height_px,
            color_space: ColorSpace::Srgb,
            alpha_mode: AlphaMode::Premultiplied,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    Srgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    Premultiplied,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub id: String,
    pub blend_mode: BlendMode,
    pub opacity: f64,
    pub visible: bool,
    pub clipping: bool,
    pub mask: Option<Mask>,
    pub source: LayerSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayerSource {
    Buffer(BlobRef),
    Adjustment(Adjustment),
    Program(StrokeProgram),
    Group(Vec<Layer>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Adjustment {
    GradientMap { stops: Vec<GradientStop> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrokeProgram {
    pub strokes: Vec<Stroke>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub brush: Brush,
    pub path: Vec<DabSample>,
    pub color: Color,
    pub opacity: f64,
    pub blend_mode: BlendMode,
    pub seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Brush {
    Round {
        radius_px: f64,
        hardness: f64,
        spacing: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DabSample {
    pub x: f64,
    pub y: f64,
    pub pressure: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    pub source: MaskSource,
    pub blob: BlobRef,
    pub invert: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskSource {
    Alpha,
    Luminance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRef {
    pub hash: ContentHash,
}

impl BlobRef {
    pub const fn new(hash: ContentHash) -> Self {
        Self { hash }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn parse(hash: impl AsRef<str>) -> Result<Self, ZpxError> {
        let hash = hash.as_ref();
        if is_lower_hex_sha256(hash) {
            Ok(Self(hash.to_owned()))
        } else {
            Err(ZpxError::new(
                "content hash must be exactly 64 lowercase hex characters",
            ))
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(zenith_session::object_hash(bytes))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_lower_hex_sha256(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
