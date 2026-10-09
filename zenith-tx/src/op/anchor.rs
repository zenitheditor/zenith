//! The [`super::Op::SetAnchor`] body: set or clear the anchor placement
//! attributes of one node.

use super::layout::{LayoutDim, nullable};

/// The body of [`super::Op::SetAnchor`].
///
/// Every attribute field is tri-state: omit it to leave the attribute
/// unchanged, pass `null` to remove it, pass a value to set it. The fields
/// match the `.zen` attributes `anchor`, `anchor-zone`, `anchor-sibling`,
/// `anchor-parent`, `anchor-edge`, and `anchor-gap`.
///
/// JSON shape:
/// `{"op":"set_anchor","node":"card.b","anchor_sibling":"card.a","anchor_edge":"below","anchor_gap":14}`.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Default)]
pub struct AnchorEdit {
    /// The stable node `id` to target.
    pub node: String,
    /// 9-point anchor: `top-left`, `top-center`, `top-right`, `center-left`,
    /// `center`, `center-right`, `bottom-left`, `bottom-center`, or
    /// `bottom-right`.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub anchor: Option<Option<String>>,
    /// Safe-zone id on the node's page that the anchor aligns inside.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub anchor_zone: Option<Option<String>>,
    /// Id of a sibling in the same container that the anchor references.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub anchor_sibling: Option<Option<String>>,
    /// `true` aligns the anchor inside the parent container's box.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub anchor_parent: Option<Option<bool>>,
    /// Edge placement next to the sibling: `above`, `below`, `before`, or
    /// `after`.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub anchor_edge: Option<Option<String>>,
    /// Gap between the node and the sibling edge: a px number, or a
    /// `"(px)N"` / `"(pt)N"` string.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pub anchor_gap: Option<Option<LayoutDim>>,
}

impl AnchorEdit {
    /// `true` when the edit names no attribute.
    pub(crate) fn is_empty(&self) -> bool {
        self.anchor.is_none()
            && self.anchor_zone.is_none()
            && self.anchor_sibling.is_none()
            && self.anchor_parent.is_none()
            && self.anchor_edge.is_none()
            && self.anchor_gap.is_none()
    }

    /// `true` when the edit sets at least one attribute to a value.
    pub(crate) fn writes_value(&self) -> bool {
        matches!(self.anchor, Some(Some(_)))
            || matches!(self.anchor_zone, Some(Some(_)))
            || matches!(self.anchor_sibling, Some(Some(_)))
            || matches!(self.anchor_parent, Some(Some(_)))
            || matches!(self.anchor_edge, Some(Some(_)))
            || matches!(self.anchor_gap, Some(Some(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_null_and_value_are_distinct() {
        let e: AnchorEdit = serde_json::from_str(
            r#"{"node":"n","anchor":null,"anchor_edge":"below","anchor_gap":"(pt)6"}"#,
        )
        .expect("parses");
        assert_eq!(e.anchor, Some(None));
        assert_eq!(e.anchor_edge, Some(Some("below".to_owned())));
        assert_eq!(
            e.anchor_gap,
            Some(Some(LayoutDim::Text("(pt)6".to_owned())))
        );
        assert_eq!(e.anchor_sibling, None);
        assert!(!e.is_empty());
        assert!(e.writes_value());
        let back = serde_json::to_value(&e).expect("serializes");
        assert!(back.get("anchor").is_some_and(serde_json::Value::is_null));
        assert!(back.get("anchor_sibling").is_none());
    }

    #[test]
    fn clear_only_edit_writes_no_value() {
        let e: AnchorEdit =
            serde_json::from_str(r#"{"node":"n","anchor":null,"anchor_gap":null}"#).expect("ok");
        assert!(!e.is_empty());
        assert!(!e.writes_value());
        assert!(AnchorEdit::default().is_empty());
    }
}
