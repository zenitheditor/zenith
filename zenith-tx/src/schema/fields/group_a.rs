//! Field schemas for the first half of the op set (paint/geometry/path ops).
//! Assembled with `group_b` by `super::op_fields`; op names are unique across
//! the two groups so match order is irrelevant.

use super::OpFieldSchema;

/// Field schemas for the ops handled by this group; `None` for any other name.
pub(super) fn op_fields(name: &str) -> Option<&'static [OpFieldSchema]> {
    match name {
        "set_text_align" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "align",
                    ty: "enum: start|center|end|justify",
                    required: true,
                },
            ];
            Some(F)
        }
        "move_forward" | "move_backward" | "move_to_front" | "move_to_back" | "remove_node"
        | "detach_pattern" => {
            static F: &[OpFieldSchema] = &[OpFieldSchema {
                name: "node",
                ty: "node id",
                required: true,
            }];
            Some(F)
        }
        "set_fill" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "fill",
                    ty: "token ref",
                    required: true,
                },
            ];
            Some(F)
        }
        "set_fill_rule" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "fill_rule",
                    ty: "enum: nonzero|evenodd",
                    required: true,
                },
            ];
            Some(F)
        }
        "set_stroke" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "stroke",
                    ty: "token ref",
                    required: true,
                },
            ];
            Some(F)
        }
        "set_stroke_width" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "stroke_width",
                    ty: "token ref",
                    required: true,
                },
            ];
            Some(F)
        }
        "set_visible" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "visible",
                    ty: "bool",
                    required: true,
                },
            ];
            Some(F)
        }
        "set_locked" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "locked",
                    ty: "bool",
                    required: true,
                },
            ];
            Some(F)
        }
        "set_geometry" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "x",
                    ty: "px",
                    required: false,
                },
                OpFieldSchema {
                    name: "y",
                    ty: "px",
                    required: false,
                },
                OpFieldSchema {
                    name: "w",
                    ty: "px | \"hug\" | \"fill\"",
                    required: false,
                },
                OpFieldSchema {
                    name: "h",
                    ty: "px | \"hug\" | \"fill\"",
                    required: false,
                },
                OpFieldSchema {
                    name: "rotate",
                    ty: "f64",
                    required: false,
                },
            ];
            Some(F)
        }
        "set_points" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "points",
                    ty: "[{x:f64,y:f64}]",
                    required: true,
                },
            ];
            Some(F)
        }
        "set_path_anchors" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "anchors",
                    ty: "[{x,y,kind?,in_x?,in_y?,out_x?,out_y?}]",
                    required: true,
                },
            ];
            Some(F)
        }
        "set_path_anchor_kind" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "anchor_index",
                    ty: "usize",
                    required: true,
                },
                OpFieldSchema {
                    name: "kind",
                    ty: "null | enum/future string: corner|smooth|symmetric",
                    required: false,
                },
            ];
            Some(F)
        }
        "remove_path_anchor" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "anchor_index",
                    ty: "usize",
                    required: true,
                },
                OpFieldSchema {
                    name: "subpath_index",
                    ty: "usize",
                    required: false,
                },
            ];
            Some(F)
        }
        "insert_path_anchor" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "segment_index",
                    ty: "usize",
                    required: true,
                },
                OpFieldSchema {
                    name: "t",
                    ty: "f64 0..=1",
                    required: true,
                },
            ];
            Some(F)
        }
        "insert_path_anchor_at_point" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "x",
                    ty: "px",
                    required: true,
                },
                OpFieldSchema {
                    name: "y",
                    ty: "px",
                    required: true,
                },
                OpFieldSchema {
                    name: "tolerance",
                    ty: "px",
                    required: true,
                },
            ];
            Some(F)
        }
        "move_path_anchor" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "anchor_index",
                    ty: "usize",
                    required: true,
                },
                OpFieldSchema {
                    name: "dx",
                    ty: "px",
                    required: true,
                },
                OpFieldSchema {
                    name: "dy",
                    ty: "px",
                    required: true,
                },
            ];
            Some(F)
        }
        "move_path_handle" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "anchor_index",
                    ty: "usize",
                    required: true,
                },
                OpFieldSchema {
                    name: "handle",
                    ty: "enum: in|out",
                    required: true,
                },
                OpFieldSchema {
                    name: "dx",
                    ty: "px",
                    required: true,
                },
                OpFieldSchema {
                    name: "dy",
                    ty: "px",
                    required: true,
                },
            ];
            Some(F)
        }
        "simplify_path_anchors" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "tolerance",
                    ty: "px",
                    required: true,
                },
            ];
            Some(F)
        }
        "transform_path_anchors" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "transform",
                    ty: r#"{mode:"translate",dx,dy} | {mode:"rotate",angle_degrees,cx,cy} | {mode:"reflect",x1,y1,x2,y2}"#,
                    required: true,
                },
            ];
            Some(F)
        }
        "snap_path_anchors" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "target",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "tolerance",
                    ty: "px",
                    required: true,
                },
            ];
            Some(F)
        }
        "make_path_symmetric" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "id_prefix",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "count",
                    ty: "usize 2..=72",
                    required: true,
                },
                OpFieldSchema {
                    name: "cx",
                    ty: "px",
                    required: true,
                },
                OpFieldSchema {
                    name: "cy",
                    ty: "px",
                    required: true,
                },
                OpFieldSchema {
                    name: "start_angle_degrees",
                    ty: "f64 degrees",
                    required: false,
                },
                OpFieldSchema {
                    name: "mirror",
                    ty: "bool (dihedral mirror symmetry; default false)",
                    required: false,
                },
            ];
            Some(F)
        }
        "path_boolean" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "target",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "new_id",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "operation",
                    ty: r#""union" | "intersect" | "subtract" | "exclude""#,
                    required: true,
                },
                OpFieldSchema {
                    name: "tolerance",
                    ty: "px",
                    required: true,
                },
            ];
            Some(F)
        }
        "add_node" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "parent",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "source",
                    ty: ".zen fragment",
                    required: true,
                },
                OpFieldSchema {
                    name: "position",
                    ty: r#"{at:"last"} | {at:"first"} | {at:"index",index:N} | {at:"before",id:"<sibling-id>"} | {at:"after",id:"<sibling-id>"}"#,
                    required: false,
                },
            ];
            Some(F)
        }
        "add_path" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "parent",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "id",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "position",
                    ty: r#"{at:"last"} | {at:"first"} | {at:"index",index:N} | {at:"before",id:"<sibling-id>"} | {at:"after",id:"<sibling-id>"}"#,
                    required: false,
                },
                OpFieldSchema {
                    name: "closed",
                    ty: "bool",
                    required: false,
                },
                OpFieldSchema {
                    name: "anchors",
                    ty: "[{x,y,kind?,in_x?,in_y?,out_x?,out_y?}]",
                    required: false,
                },
                OpFieldSchema {
                    name: "subpaths",
                    ty: "[{closed?,anchors:[{x,y,kind?,in_x?,in_y?,out_x?,out_y?}]}]",
                    required: false,
                },
            ];
            Some(F)
        }
        _ => None,
    }
}
