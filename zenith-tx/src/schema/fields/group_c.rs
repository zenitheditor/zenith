//! Field schemas for the auto-layout ops. Assembled with `group_a` and
//! `group_b` by `super::op_fields`; op names are unique across the groups.

use super::OpFieldSchema;

/// Field schemas for the ops handled by this group; `None` for any other name.
pub(super) fn op_fields(name: &str) -> Option<&'static [OpFieldSchema]> {
    match name {
        "set_layout" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "layout",
                    ty: "enum: absolute|row|column|grid | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "gap",
                    ty: "px | \"(px)N\" | dimension token id | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "wrap_gap",
                    ty: "px | \"(px)N\" | dimension token id | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "padding",
                    ty: "px | \"(px)N\" | dimension token id | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "padding_x",
                    ty: "px | \"(px)N\" | dimension token id | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "padding_y",
                    ty: "px | \"(px)N\" | dimension token id | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "padding_top",
                    ty: "px | \"(px)N\" | dimension token id | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "padding_right",
                    ty: "px | \"(px)N\" | dimension token id | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "padding_bottom",
                    ty: "px | \"(px)N\" | dimension token id | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "padding_left",
                    ty: "px | \"(px)N\" | dimension token id | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "justify",
                    ty: "enum: start|center|end|space-between | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "align",
                    ty: "enum: start|center|end|stretch | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "wrap",
                    ty: "bool | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "clip",
                    ty: "bool | null (frame)",
                    required: false,
                },
                OpFieldSchema {
                    name: "position",
                    ty: "enum: auto|absolute | null (box node)",
                    required: false,
                },
                OpFieldSchema {
                    name: "min_w",
                    ty: "px | \"(px)N\" | dimension token id | null (box node)",
                    required: false,
                },
                OpFieldSchema {
                    name: "max_w",
                    ty: "px | \"(px)N\" | dimension token id | null (box node)",
                    required: false,
                },
                OpFieldSchema {
                    name: "min_h",
                    ty: "px | \"(px)N\" | dimension token id | null (box node)",
                    required: false,
                },
                OpFieldSchema {
                    name: "max_h",
                    ty: "px | \"(px)N\" | dimension token id | null (box node)",
                    required: false,
                },
            ];
            Some(F)
        }
        _ => None,
    }
}
