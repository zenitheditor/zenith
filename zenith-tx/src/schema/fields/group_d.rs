//! Field schemas for the delta and anchor ops an editor gesture emits.
//! Assembled with the other groups by `super::op_fields`; op names are
//! unique across the groups.

use super::OpFieldSchema;

const NODE: OpFieldSchema = OpFieldSchema {
    name: "node",
    ty: "node id",
    required: true,
};

const fn delta(name: &'static str) -> OpFieldSchema {
    OpFieldSchema {
        name,
        ty: "px delta (finite, omit keeps)",
        required: false,
    }
}

const fn nullable(name: &'static str, ty: &'static str) -> OpFieldSchema {
    OpFieldSchema {
        name,
        ty,
        required: false,
    }
}

/// Field schemas for the ops handled by this group; `None` for any other name.
pub(super) fn op_fields(name: &str) -> Option<&'static [OpFieldSchema]> {
    match name {
        "nudge_geometry" => {
            static F: &[OpFieldSchema] = &[
                NODE,
                delta("dx"),
                delta("dy"),
                delta("dw"),
                delta("dh"),
                OpFieldSchema {
                    name: "detach",
                    ty: "bool, default false (true replaces a token-bound axis with px)",
                    required: false,
                },
            ];
            Some(F)
        }
        "set_anchor" => {
            static F: &[OpFieldSchema] = &[
                NODE,
                nullable(
                    "anchor",
                    "enum: top-left|top-center|top-right|center-left|center|center-right|\
                     bottom-left|bottom-center|bottom-right | null (omit keeps, null removes)",
                ),
                nullable(
                    "anchor_zone",
                    "safe-zone id | null (omit keeps, null removes)",
                ),
                nullable(
                    "anchor_sibling",
                    "sibling node id | null (omit keeps, null removes)",
                ),
                nullable("anchor_parent", "bool | null (omit keeps, null removes)"),
                nullable(
                    "anchor_edge",
                    "enum: above|below|before|after | null (omit keeps, null removes)",
                ),
                nullable(
                    "anchor_gap",
                    "px | \"(px)N\" | \"(pt)N\" | null (omit keeps, null removes)",
                ),
            ];
            Some(F)
        }
        "nudge_anchor_gap" => {
            static F: &[OpFieldSchema] = &[NODE, delta("dx"), delta("dy")];
            Some(F)
        }
        "set_node_token" => {
            static F: &[OpFieldSchema] = &[
                NODE,
                OpFieldSchema {
                    name: "property",
                    ty: "enum: radius|font-family|font-size|font-weight",
                    required: true,
                },
                nullable("token", "token ref | null (null or omitted removes)"),
            ];
            Some(F)
        }
        "set_span_text" => {
            static F: &[OpFieldSchema] = &[
                NODE,
                OpFieldSchema {
                    name: "span",
                    ty: "usize (0-based span index)",
                    required: true,
                },
                OpFieldSchema {
                    name: "text",
                    ty: "string",
                    required: true,
                },
            ];
            Some(F)
        }
        "detach_anchor" => {
            static F: &[OpFieldSchema] = &[NODE];
            Some(F)
        }
        "nudge_line_points" => {
            static F: &[OpFieldSchema] =
                &[NODE, delta("dx1"), delta("dy1"), delta("dx2"), delta("dy2")];
            Some(F)
        }
        _ => None,
    }
}
