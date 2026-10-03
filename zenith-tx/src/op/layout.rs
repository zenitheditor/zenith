//! Auto-layout payloads: the [`super::Op::SetLayout`] body, its dimension
//! input, and the `w`/`h` input of [`super::Op::SetGeometry`].

use serde::{Deserialize, Deserializer};

/// A `w`/`h` value of [`super::Op::SetGeometry`]: a px number, or the size
/// keyword `"hug"` / `"fill"`.
///
/// JSON shape: `120` or `"hug"`.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum SizeInput {
    /// A size in document pixels.
    Px(f64),
    /// A size keyword: `hug` (size to content) or `fill` (take the free space).
    Keyword(String),
}

impl From<f64> for SizeInput {
    fn from(v: f64) -> Self {
        Self::Px(v)
    }
}

impl From<&str> for SizeInput {
    fn from(v: &str) -> Self {
        Self::Keyword(v.to_owned())
    }
}

/// A layout dimension of [`LayoutEdit`]: a px number, a canonical dimension
/// string (`"(px)8"`, `"(pt)6"`), or a dimension token id (`"space.md"`).
///
/// JSON shape: `8`, `"(px)8"`, or `"space.md"`.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum LayoutDim {
    /// A size in document pixels.
    Px(f64),
    /// A `(unit)value` string or a dimension token id.
    Text(String),
}

impl From<f64> for LayoutDim {
    fn from(v: f64) -> Self {
        Self::Px(v)
    }
}

impl From<&str> for LayoutDim {
    fn from(v: &str) -> Self {
        Self::Text(v.to_owned())
    }
}

/// Deserialize a field where an absent key means "leave unchanged" and an
/// explicit `null` means "clear": absent → `None`, `null` → `Some(None)`,
/// a value → `Some(Some(v))`. Pair with `#[serde(default)]`.
pub(super) fn nullable<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(de).map(Some)
}

/// The body of [`super::Op::SetLayout`]: set or clear auto-layout attributes
/// on one node.
///
/// Every attribute field is tri-state: omit it to leave the attribute
/// unchanged, pass `null` to clear it, pass a value to set it. Container
/// fields (`layout` … `clip`) apply to `frame` nodes only. Item fields
/// (`position`, `min_w` … `max_h`) apply to every box node and `instance`.
///
/// JSON shape: `{"op":"set_layout","node":"card","layout":"column","gap":16,"padding":"space.md"}`.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Default)]
pub struct LayoutEdit {
    /// The stable node `id` to target.
    pub node: String,
    /// Frame layout mode: `absolute`, `row`, `column`, or `grid`.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub layout: Option<Option<String>>,
    /// Main-axis gap between children.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub gap: Option<Option<LayoutDim>>,
    /// Gap between wrapped lines.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub wrap_gap: Option<Option<LayoutDim>>,
    /// Uniform inset on all four sides.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub padding: Option<Option<LayoutDim>>,
    /// Left + right inset.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub padding_x: Option<Option<LayoutDim>>,
    /// Top + bottom inset.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub padding_y: Option<Option<LayoutDim>>,
    /// Top inset.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub padding_top: Option<Option<LayoutDim>>,
    /// Right inset.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub padding_right: Option<Option<LayoutDim>>,
    /// Bottom inset.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub padding_bottom: Option<Option<LayoutDim>>,
    /// Left inset.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub padding_left: Option<Option<LayoutDim>>,
    /// Main-axis distribution: `start`, `center`, `end`, or `space-between`.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub justify: Option<Option<String>>,
    /// Cross-axis placement: `start`, `center`, `end`, or `stretch`.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub align: Option<Option<String>>,
    /// Wrap children onto new lines when the main axis is full.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub wrap: Option<Option<bool>>,
    /// Clip children to the frame box.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub clip: Option<Option<bool>>,
    /// Item placement: `auto` (in flow) or `absolute` (keeps its x/y).
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub position: Option<Option<String>>,
    /// Item minimum width.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub min_w: Option<Option<LayoutDim>>,
    /// Item maximum width.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_w: Option<Option<LayoutDim>>,
    /// Item minimum height.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub min_h: Option<Option<LayoutDim>>,
    /// Item maximum height.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_h: Option<Option<LayoutDim>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_null_and_value_are_distinct() {
        let e: LayoutEdit =
            serde_json::from_str(r#"{"node":"f","gap":null,"padding":8,"justify":"center"}"#)
                .expect("parses");
        assert_eq!(e.gap, Some(None));
        assert_eq!(e.padding, Some(Some(LayoutDim::Px(8.0))));
        assert_eq!(e.justify, Some(Some("center".to_owned())));
        assert_eq!(e.layout, None);
        let back = serde_json::to_value(&e).expect("serializes");
        assert!(back.get("gap").is_some_and(serde_json::Value::is_null));
        assert!(back.get("layout").is_none());
    }

    #[test]
    fn size_input_reads_number_or_keyword() {
        let n: SizeInput = serde_json::from_str("12.5").expect("number");
        let k: SizeInput = serde_json::from_str(r#""hug""#).expect("keyword");
        assert_eq!(n, SizeInput::Px(12.5));
        assert_eq!(k, SizeInput::Keyword("hug".to_owned()));
    }
}
