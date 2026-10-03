//! Field schemas for the second half of the op set (opacity/text/structure/
//! document/token/recipe ops). Assembled with `group_a` by `super::op_fields`;
//! op names are unique across the two groups so match order is irrelevant.

use super::OpFieldSchema;

/// Field schemas for the ops handled by this group; `None` for any other name.
pub(super) fn op_fields(name: &str) -> Option<&'static [OpFieldSchema]> {
    match name {
        "set_opacity" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "opacity",
                    ty: "f64",
                    required: true,
                },
            ];
            Some(F)
        }
        "replace_text" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "spans",
                    ty: "[{text,fill?,font_weight?,italic?,…}]",
                    required: true,
                },
            ];
            Some(F)
        }
        "duplicate_node" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "new_id",
                    ty: "string",
                    required: true,
                },
            ];
            Some(F)
        }
        "duplicate_page" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "page",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "new_id",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "id_suffix",
                    ty: "string",
                    required: true,
                },
            ];
            Some(F)
        }
        "group" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node_ids",
                    ty: "node-id[]",
                    required: true,
                },
                OpFieldSchema {
                    name: "group_id",
                    ty: "string",
                    required: true,
                },
            ];
            Some(F)
        }
        "ungroup" => {
            static F: &[OpFieldSchema] = &[OpFieldSchema {
                name: "group_id",
                ty: "node id",
                required: true,
            }];
            Some(F)
        }
        "reparent" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "new_parent",
                    ty: "node id",
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
        "align_nodes" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node_ids",
                    ty: "node-id[]",
                    required: true,
                },
                OpFieldSchema {
                    name: "align",
                    ty: "enum: left|hcenter|right|top|vcenter|bottom",
                    required: true,
                },
                OpFieldSchema {
                    name: "anchor",
                    ty: "string",
                    required: false,
                },
            ];
            Some(F)
        }
        "set_text_overflow" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node_id",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "overflow",
                    // Must match zenith-core `TEXT_OVERFLOWS` / `CODE_OVERFLOWS`
                    // (checked by `set_text_overflow_schema_matches_core_enums`).
                    ty: "enum: clip|visible|fit|autofit (code: clip|visible)",
                    required: true,
                },
            ];
            Some(F)
        }
        "add_page" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "id",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "w",
                    ty: "px",
                    required: true,
                },
                OpFieldSchema {
                    name: "h",
                    ty: "px",
                    required: true,
                },
                OpFieldSchema {
                    name: "background",
                    ty: "token ref",
                    required: false,
                },
                OpFieldSchema {
                    name: "index",
                    ty: "i64",
                    required: false,
                },
            ];
            Some(F)
        }
        "delete_page" => {
            static F: &[OpFieldSchema] = &[OpFieldSchema {
                name: "page",
                ty: "node id",
                required: true,
            }];
            Some(F)
        }
        "reorder_pages" => {
            static F: &[OpFieldSchema] = &[OpFieldSchema {
                name: "order",
                ty: "node-id[]",
                required: true,
            }];
            Some(F)
        }
        "add_asset" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "id",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "kind",
                    ty: "enum: image|svg|font",
                    required: true,
                },
                OpFieldSchema {
                    name: "src",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "sha256",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "producer_kind",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "producer_source",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "ai_prompt",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "ai_model",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "ai_provider",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "ai_seed",
                    ty: "integer",
                    required: false,
                },
                OpFieldSchema {
                    name: "ai_generation_date",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "ai_license",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "ai_source_rights",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "ai_safety_status",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "ai_reuse_policy",
                    ty: "string",
                    required: false,
                },
            ];
            Some(F)
        }
        "set_asset" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node_id",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "asset_id",
                    ty: "string",
                    required: true,
                },
            ];
            Some(F)
        }
        "distribute_nodes" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node_ids",
                    ty: "node-id[]",
                    required: true,
                },
                OpFieldSchema {
                    name: "axis",
                    ty: "enum: horizontal|vertical",
                    required: true,
                },
            ];
            Some(F)
        }
        "create_token" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "id",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "type",
                    ty: "enum: color|dimension|number|fontFamily|fontWeight|shadow|filter|gradient|mask",
                    required: true,
                },
                OpFieldSchema {
                    name: "value",
                    ty: "string (required for scalar types)",
                    required: false,
                },
                OpFieldSchema {
                    name: "set",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "layers",
                    ty: "array of {dx,dy,blur,color} (shadow only)",
                    required: false,
                },
                OpFieldSchema {
                    name: "filter_ops",
                    ty: "array of {kind,…} (filter only)",
                    required: false,
                },
                OpFieldSchema {
                    name: "stops",
                    ty: "array of {offset,color} (gradient; ≥2)",
                    required: false,
                },
                OpFieldSchema {
                    name: "angle",
                    ty: "number degrees (linear gradient)",
                    required: false,
                },
                OpFieldSchema {
                    name: "radial",
                    ty: "bool (gradient)",
                    required: false,
                },
                OpFieldSchema {
                    name: "center_x",
                    ty: "number 0..1 (radial gradient)",
                    required: false,
                },
                OpFieldSchema {
                    name: "center_y",
                    ty: "number 0..1 (radial gradient)",
                    required: false,
                },
                OpFieldSchema {
                    name: "radius",
                    ty: "number (radial fraction or rounded-mask px)",
                    required: false,
                },
                OpFieldSchema {
                    name: "shape",
                    ty: "enum: rect|rounded|ellipse (mask)",
                    required: false,
                },
                OpFieldSchema {
                    name: "feather",
                    ty: "number px (mask)",
                    required: false,
                },
                OpFieldSchema {
                    name: "invert",
                    ty: "bool (mask)",
                    required: false,
                },
            ];
            Some(F)
        }
        "create_style" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "id",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "properties",
                    ty: "map of style-key → token id",
                    required: false,
                },
            ];
            Some(F)
        }
        "delete_style" => {
            static F: &[OpFieldSchema] = &[OpFieldSchema {
                name: "id",
                ty: "string",
                required: true,
            }];
            Some(F)
        }
        "create_master" => {
            static F: &[OpFieldSchema] = &[OpFieldSchema {
                name: "id",
                ty: "string",
                required: true,
            }];
            Some(F)
        }
        "delete_master" => {
            static F: &[OpFieldSchema] = &[OpFieldSchema {
                name: "id",
                ty: "string",
                required: true,
            }];
            Some(F)
        }
        "set_page_master" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "page",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "master",
                    ty: "string or null",
                    required: false,
                },
            ];
            Some(F)
        }
        "update_token_value" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "id",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "value",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "set",
                    ty: "string or null (omit keeps, null removes)",
                    required: false,
                },
            ];
            Some(F)
        }
        "set_style_property" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "style_id",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "property",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "value",
                    ty: "token ref",
                    required: true,
                },
            ];
            Some(F)
        }
        "set_text_direction" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "direction",
                    ty: "enum: ltr|rtl",
                    required: true,
                },
            ];
            Some(F)
        }
        "find_replace_text" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "find",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "replace",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: false,
                },
            ];
            Some(F)
        }
        "set_page_size" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "page",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "w",
                    ty: "px",
                    required: true,
                },
                OpFieldSchema {
                    name: "h",
                    ty: "px",
                    required: true,
                },
            ];
            Some(F)
        }
        "align_to_edge" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "node",
                    ty: "node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "edge",
                    ty: "enum: left|right|top|bottom|hcenter|vcenter",
                    required: true,
                },
                OpFieldSchema {
                    name: "margin",
                    ty: "f64",
                    required: false,
                },
            ];
            Some(F)
        }
        "create_recipe" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "id",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "kind",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "seed",
                    ty: "i64",
                    required: false,
                },
                OpFieldSchema {
                    name: "generator",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "bounds",
                    ty: "node id",
                    required: false,
                },
                OpFieldSchema {
                    name: "detached",
                    ty: "bool",
                    required: false,
                },
            ];
            Some(F)
        }
        "update_recipe" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "id",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "kind",
                    ty: "string",
                    required: true,
                },
                OpFieldSchema {
                    name: "seed",
                    ty: "i64",
                    required: false,
                },
                OpFieldSchema {
                    name: "generator",
                    ty: "string",
                    required: false,
                },
                OpFieldSchema {
                    name: "bounds",
                    ty: "node id",
                    required: false,
                },
                OpFieldSchema {
                    name: "detached",
                    ty: "bool",
                    required: false,
                },
            ];
            Some(F)
        }
        "set_default" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "page",
                    ty: "string or null (page id; omit for document scope)",
                    required: false,
                },
                OpFieldSchema {
                    name: "kind",
                    ty: "string (node kind, e.g. text|shape|rect)",
                    required: true,
                },
                OpFieldSchema {
                    name: "style",
                    ty: "string (style id)",
                    required: true,
                },
                OpFieldSchema {
                    name: "text_style",
                    ty: "string or null (style id; shape|connector only; omit keeps, null clears)",
                    required: false,
                },
            ];
            Some(F)
        }
        "remove_default" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "page",
                    ty: "string or null (page id; omit for document scope)",
                    required: false,
                },
                OpFieldSchema {
                    name: "kind",
                    ty: "string (node kind)",
                    required: true,
                },
            ];
            Some(F)
        }
        "delete_recipe" => {
            static F: &[OpFieldSchema] = &[OpFieldSchema {
                name: "id",
                ty: "string",
                required: true,
            }];
            Some(F)
        }
        _ => None,
    }
}
