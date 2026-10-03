//! Agent-readable default values for attributes whose absence has a defined
//! meaning. Attributes without an entry have no default (absent = unset).

/// Box-node kinds that carry the auto-layout item attributes.
const BOX_KINDS: &[&str] = &[
    "rect", "ellipse", "text", "code", "frame", "group", "image", "field", "toc", "table", "shape",
    "pattern", "chart", "mesh", "instance",
];

/// Return the default of attribute `name` on node kind `kind`, as the value an
/// author would write, or `None` when the attribute has no stated default.
pub fn attribute_default(kind: &str, name: &str) -> Option<&'static str> {
    match (kind, name) {
        ("frame", "layout") => Some("absolute"),
        ("frame", "gap") => Some("frame style `gap`, else (px)0"),
        ("frame", "wrap-gap") => Some("gap"),
        ("frame", "padding") => Some("frame style `padding`, else (px)0"),
        ("frame", "padding-x" | "padding-y") => Some("padding"),
        ("frame", "padding-top" | "padding-bottom") => Some("padding-y"),
        ("frame", "padding-left" | "padding-right") => Some("padding-x"),
        ("frame", "justify") => Some("start"),
        ("frame", "align") => Some("stretch"),
        ("frame", "wrap") => Some("#false"),
        ("frame", "clip") => Some("#true; #false when layout is row or column"),
        (k, "position") if BOX_KINDS.contains(&k) => Some("auto"),
        ("text" | "code", "overflow") => Some("clip"),
        ("chart", "fill") => Some(
            "text colour: style `fill`, else the ambient `.content` pair, else the best-contrast `.content` token, else black or white",
        ),
        ("chart", "stroke") => {
            Some("axis colour: style `stroke`, else the text colour mixed 50% into the backdrop")
        }
        ("chart", "stroke-width") => {
            Some("axis and gridline width: style `stroke-width`, else (px)1")
        }
        ("chart", "style") => Some(
            "the `defaults` chart row. Keys: font-family, font-size (base px), fill, stroke, stroke-width, shadow",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::node_attributes;

    #[test]
    fn frame_layout_defaults_are_listed() {
        for name in [
            "layout",
            "gap",
            "wrap-gap",
            "padding",
            "padding-x",
            "padding-y",
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
            "justify",
            "align",
            "wrap",
            "clip",
        ] {
            assert!(
                attribute_default("frame", name).is_some(),
                "frame.{name} needs a default"
            );
            assert!(
                node_attributes("frame").contains(&name),
                "frame lists {name}"
            );
        }
    }

    #[test]
    fn every_box_kind_lists_item_attrs_with_position_default() {
        for kind in BOX_KINDS {
            let attrs = node_attributes(kind);
            for name in ["min-w", "max-w", "min-h", "max-h", "position"] {
                assert!(attrs.contains(&name), "{kind} lists {name}");
            }
            assert_eq!(attribute_default(kind, "position"), Some("auto"));
        }
        assert_eq!(attribute_default("line", "position"), None);
        assert_eq!(attribute_default("rect", "fill"), None);
        assert_eq!(attribute_default("text", "overflow"), Some("clip"));
        assert_eq!(attribute_default("code", "overflow"), Some("clip"));
    }
}
