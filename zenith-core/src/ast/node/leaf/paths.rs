//! Point-based leaf structs: `polygon`, `polyline`, `path`, and the shared
//! path anchor/subpath types (`PathAnchor`, `PathSubpath`, `PathSubpathRef`,
//! `AnchorKind`).

use std::collections::BTreeMap;

use crate::ast::Span;
use crate::ast::value::{Dimension, PropertyValue};

use crate::ast::node::common::{Point, UnknownProperty};

/// A `polygon` node — a CLOSED filled shape defined by an ordered list of
/// `point` child nodes.
///
/// `polygon` supports both fill and stroke (stroke is centered in v0).
/// `fill-rule` controls the winding rule for self-intersecting fills.
/// `stroke-alignment` is parsed and preserved for future use but the stroke
/// is ALWAYS rendered centered in v0.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub fill: Option<PropertyValue>,
    pub stroke: Option<PropertyValue>,
    pub stroke_width: Option<PropertyValue>,
    /// Stroke alignment: `"center"` (default), `"inside"`, or `"outside"`.
    /// `inside`/`outside` shift closed-shape strokes; open paths stroke centered.
    pub stroke_alignment: Option<String>,
    /// `"nonzero"` (default) or `"evenodd"`.
    pub fill_rule: Option<String>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub rotate: Option<Dimension>,
    pub style: Option<String>,
    /// Ordered vertex list parsed from `point` child nodes.
    pub points: Vec<Point>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

/// A single anchor in a `path` anchor list.
///
/// `x` and `y` define the anchor point. `in_*` and `out_*` are optional Bezier
/// handles, preserved losslessly at parse time and pair-validated later.
#[derive(Debug, Clone, PartialEq)]
pub struct PathAnchor {
    pub x: Option<Dimension>,
    pub y: Option<Dimension>,
    /// Authoring intent for editor handles. This is metadata only; render
    /// geometry is derived solely from coordinates and handles.
    pub kind: Option<AnchorKind>,
    pub in_x: Option<Dimension>,
    pub in_y: Option<Dimension>,
    pub out_x: Option<Dimension>,
    pub out_y: Option<Dimension>,
}

/// One contour within a compound `path` node.
#[derive(Debug, Clone, PartialEq)]
pub struct PathSubpath {
    /// Per-contour closure. Defaults to open when absent.
    pub closed: Option<bool>,
    /// Ordered anchor list for this contour.
    pub anchors: Vec<PathAnchor>,
}

/// Borrowed view over either legacy direct anchors or an authored subpath.
#[derive(Debug, Clone, Copy)]
pub struct PathSubpathRef<'a> {
    pub closed: Option<bool>,
    pub anchors: &'a [PathAnchor],
}

struct EffectiveSubpaths<'a> {
    direct: Option<PathSubpathRef<'a>>,
    iter: Option<std::slice::Iter<'a, PathSubpath>>,
}

impl<'a> EffectiveSubpaths<'a> {
    fn new(path: &'a PathNode) -> Self {
        if path.subpaths.is_empty() {
            Self {
                direct: Some(PathSubpathRef {
                    closed: path.closed,
                    anchors: &path.anchors,
                }),
                iter: None,
            }
        } else {
            Self {
                direct: None,
                iter: Some(path.subpaths.iter()),
            }
        }
    }
}

impl<'a> Iterator for EffectiveSubpaths<'a> {
    type Item = PathSubpathRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(direct) = self.direct.take() {
            return Some(direct);
        }

        self.iter.as_mut().and_then(|iter| {
            iter.next().map(|subpath| PathSubpathRef {
                closed: subpath.closed,
                anchors: &subpath.anchors,
            })
        })
    }
}

/// Authoring intent for a path anchor.
///
/// Unknown strings are preserved for forward-compatibility; validation warns
/// but parsing and formatting remain lossless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnchorKind {
    Corner,
    Smooth,
    Symmetric,
    Unknown(String),
}

impl AnchorKind {
    /// Parse a KDL `kind` attribute value, preserving unknown values.
    pub fn from_kind_str(value: &str) -> Self {
        match value {
            "corner" => Self::Corner,
            "smooth" => Self::Smooth,
            "symmetric" => Self::Symmetric,
            other => Self::Unknown(other.to_owned()),
        }
    }

    /// Return the canonical authoring string for this anchor kind.
    pub fn kind_str(&self) -> &str {
        match self {
            Self::Corner => "corner",
            Self::Smooth => "smooth",
            Self::Symmetric => "symmetric",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

/// A `path` node — a structured Bezier path defined by ordered `anchor`
/// children with optional in/out handles.
///
/// `closed` preserves author intent for open versus closed paths. `fill-rule`
/// and `stroke-alignment` use the same value model as `polygon`.
#[derive(Debug, Clone, PartialEq)]
pub struct PathNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub closed: Option<bool>,
    pub fill: Option<PropertyValue>,
    pub stroke: Option<PropertyValue>,
    pub stroke_width: Option<PropertyValue>,
    /// Stroke alignment: `"center"` (default), `"inside"`, or `"outside"`.
    pub stroke_alignment: Option<String>,
    /// Stroke corner join style: `"miter"` (default), `"round"`, or `"bevel"`.
    pub stroke_linejoin: Option<String>,
    /// Stroke end-cap style: `"butt"` (default), `"round"`, or `"square"`.
    pub stroke_linecap: Option<String>,
    /// Positive finite miter limit for miter joins.
    pub stroke_miter_limit: Option<f64>,
    /// `"nonzero"` (default) or `"evenodd"`.
    pub fill_rule: Option<String>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub rotate: Option<Dimension>,
    pub style: Option<String>,
    /// Ordered anchor list parsed from `anchor` child nodes.
    pub anchors: Vec<PathAnchor>,
    /// Ordered compound contours parsed from `subpath` child nodes.
    pub subpaths: Vec<PathSubpath>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}

impl PathNode {
    pub fn effective_subpaths(&self) -> impl Iterator<Item = PathSubpathRef<'_>> + '_ {
        EffectiveSubpaths::new(self)
    }
}

/// A `polyline` node — an OPEN stroked path defined by an ordered list of
/// `point` child nodes.
///
/// `polyline` has stroke (required for visible output) and optional fill.
/// Unlike `polygon`, `polyline` does NOT support `stroke-alignment`.
#[derive(Debug, Clone, PartialEq)]
pub struct PolylineNode {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub fill: Option<PropertyValue>,
    pub stroke: Option<PropertyValue>,
    pub stroke_width: Option<PropertyValue>,
    /// `"nonzero"` (default) or `"evenodd"`.
    pub fill_rule: Option<String>,
    pub opacity: Option<f64>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub rotate: Option<Dimension>,
    pub style: Option<String>,
    /// Ordered vertex list parsed from `point` child nodes.
    pub points: Vec<Point>,
    /// Source declaration span, when available.
    pub source_span: Option<Span>,
    /// Unknown properties preserved for forward-compat.
    pub unknown_props: BTreeMap<String, UnknownProperty>,
}
