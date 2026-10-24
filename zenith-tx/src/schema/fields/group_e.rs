//! Field schemas for the page-order ops (`move_page`, `swap_pages`) and the
//! instance override op (`set_override`). Assembled with the other groups by
//! `super::op_fields`; op names are unique across the groups.

use super::OpFieldSchema;

/// Field schemas for the ops handled by this group; `None` for any other name.
pub(super) fn op_fields(name: &str) -> Option<&'static [OpFieldSchema]> {
    match name {
        "move_page" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "page",
                    ty: "page id",
                    required: true,
                },
                OpFieldSchema {
                    name: "position",
                    ty: "{\"at\":\"first\"|\"last\"} | {\"at\":\"index\",\"index\":N} (0-based, final order) | {\"at\":\"before\"|\"after\",\"id\":page id}",
                    required: true,
                },
            ];
            Some(F)
        }
        "swap_pages" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "a",
                    ty: "page id",
                    required: true,
                },
                OpFieldSchema {
                    name: "b",
                    ty: "page id",
                    required: true,
                },
            ];
            Some(F)
        }
        "set_override" => {
            static F: &[OpFieldSchema] = &[
                OpFieldSchema {
                    name: "instance",
                    ty: "instance node id",
                    required: true,
                },
                OpFieldSchema {
                    name: "ref",
                    ty: "component-local child id (as in override ref=\"…\")",
                    required: true,
                },
                OpFieldSchema {
                    name: "text",
                    ty: "string | null (text child; one span, keeps the first span's attributes)",
                    required: false,
                },
                OpFieldSchema {
                    name: "fill",
                    ty: "color token id | null",
                    required: false,
                },
                OpFieldSchema {
                    name: "visible",
                    ty: "bool | null",
                    required: false,
                },
            ];
            Some(F)
        }
        _ => None,
    }
}
