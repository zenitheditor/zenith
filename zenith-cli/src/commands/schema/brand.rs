//! The brand-kit and markdown-block schema surfaces.

use crate::commands::serialize_pretty;
use crate::json_types::{SchemaBrandChildNode, SchemaBrandDiagCode, SchemaBrandOutput};

pub fn brand(json: bool) -> (String, u8) {
    const SUMMARY: &str = "Declare the allowed palette, fonts, and weights for this document; \
        resolved token values outside the contract emit Warnings that can be elevated to \
        blocking Errors for a CI gate.";

    const PLACEMENT: &str = "Top-level child of the root `zenith version=1 { … }` node, \
        sibling of `tokens`, `assets`, and `document`. At most one `brand { … }` block \
        per document.";

    const ABSENT_MEANS: &str = "An absent child node means that category is UNCONSTRAINED — \
        omitting `colors` allows any color; omitting `fonts` allows any font family; \
        omitting `weights` allows any weight. A completely empty `brand {}` block constrains \
        nothing.";

    const CHILD_NODES: &[SchemaBrandChildNode] = &[
        SchemaBrandChildNode {
            node: "colors",
            syntax: r##"colors "#rrggbb" "#rrggbb" …"##,
            description: "Allowed sRGB hex colors (case-insensitive). Color tokens and the \
                sRGB-equivalent of CMYK tokens are compared against this list. Any resolved \
                color token whose value is absent from this set emits `brand.color_off_palette`.",
        },
        SchemaBrandChildNode {
            node: "fonts",
            syntax: r#"fonts "Family Name" "Another Family" …"#,
            description: "Allowed font family names. Any resolved fontFamily token whose value \
                is not in this set emits `brand.font_not_allowed`.",
        },
        SchemaBrandChildNode {
            node: "weights",
            syntax: "weights 400 700 …",
            description: "Allowed font weights as bare integers (100–900 in multiples of 100). \
                Any resolved fontWeight token whose value is not in this set emits \
                `brand.weight_not_allowed`.",
        },
    ];

    const DIAG_CODES: &[SchemaBrandDiagCode] = &[
        SchemaBrandDiagCode {
            code: "brand.color_off_palette",
            severity: "warning",
            summary: "Resolved color token value is not in the declared brand palette.",
        },
        SchemaBrandDiagCode {
            code: "brand.font_not_allowed",
            severity: "warning",
            summary: "Resolved fontFamily token value is not in the declared brand font list.",
        },
        SchemaBrandDiagCode {
            code: "brand.weight_not_allowed",
            severity: "warning",
            summary: "Resolved fontWeight token value is not in the declared brand weight list.",
        },
    ];

    const EXAMPLE: &str = concat!(
        "zenith version=1 {\n",
        "  brand {\n",
        "    colors \"#0b1f33\" \"#1b6cf0\" \"#ffffff\"\n",
        "    fonts \"Noto Sans\"\n",
        "    weights 400 700\n",
        "  }\n",
        "  tokens format=\"zenith-token-v1\" {\n",
        "    token id=\"color.primary\" type=\"color\" value=\"#1b6cf0\"\n",
        "    token id=\"color.bg\"      type=\"color\" value=\"#ffffff\"\n",
        "    token id=\"font.body\"     type=\"fontFamily\" value=\"Noto Sans\"\n",
        "    token id=\"weight.bold\"   type=\"fontWeight\" value=700\n",
        "  }\n",
        "  document id=\"doc\" title=\"Brand demo\" {}\n",
        "}\n",
        "\n",
        "# CI gate — make off-contract values block the build:\n",
        "#   zenith validate doc.zen --deny brand.color_off_palette\n",
        "#\n",
        "# Or declare the policy in-file:\n",
        "#   diagnostics { deny \"brand.color_off_palette\" }"
    );

    if json {
        let child_nodes: Vec<SchemaBrandChildNode> = CHILD_NODES
            .iter()
            .map(|n| SchemaBrandChildNode {
                node: n.node,
                syntax: n.syntax,
                description: n.description,
            })
            .collect();
        let diag_codes: Vec<SchemaBrandDiagCode> = DIAG_CODES
            .iter()
            .map(|d| SchemaBrandDiagCode {
                code: d.code,
                severity: d.severity,
                summary: d.summary,
            })
            .collect();
        let out = SchemaBrandOutput {
            schema: "zenith-schema-v1",
            summary: SUMMARY.to_owned(),
            placement: PLACEMENT,
            child_nodes,
            absent_means: ABSENT_MEANS,
            diagnostic_codes: diag_codes,
            example: EXAMPLE,
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = format!("brand: {SUMMARY}\n");

        text.push_str(&format!("\nPlacement:\n  {PLACEMENT}\n"));

        text.push_str("\nChild nodes (all optional):\n");
        for node in CHILD_NODES {
            text.push_str(&format!(
                "  {:<8}  syntax:  {}\n           {}\n",
                node.node, node.syntax, node.description
            ));
        }

        text.push_str(&format!("\nAbsent-child rule:\n  {ABSENT_MEANS}\n"));

        text.push_str("\nDiagnostic codes (Warning by default):\n");
        let col = DIAG_CODES.iter().map(|d| d.code.len()).max().unwrap_or(0);
        for d in DIAG_CODES {
            text.push_str(&format!(
                "  {:<col$}  —  {}\n",
                d.code,
                d.summary,
                col = col,
            ));
        }

        text.push_str(
            "\nCI gate:\n  \
            Elevate to blocking Errors with `--deny <code>` on the CLI:\n    \
            zenith validate doc.zen --deny brand.color_off_palette\n  \
            Or declare the policy in-file (cross-reference `zenith schema diagnostics`):\n    \
            diagnostics { deny \"brand.color_off_palette\" }\n",
        );

        text.push_str(&format!("\nExample:\n  {}", EXAMPLE.replace('\n', "\n  ")));
        (text, 0)
    }
}

/// `zenith schema block`: role vocabulary, properties, scopes, and cascade for
/// the `block role="…"` declaration.
///
/// Returns `(stdout, exit_code)`.
pub fn block(json: bool) -> (String, u8) {
    // Single source of truth lives in zenith-core; no need to duplicate it here.
    let role_vocab = zenith_core::BLOCK_ROLE_VOCAB;

    const PROPS: &[(&str, &str)] = &[
        (
            "role",
            "string — required; the markdown block role to target (see vocab above)",
        ),
        (
            "font-family",
            "token ref or literal string — override font family for this role",
        ),
        (
            "font-size",
            "token ref, (px) literal, or dimension — override font size",
        ),
        (
            "font-weight",
            "token ref or literal — override font weight (100–900)",
        ),
        (
            "fill",
            "token ref or color literal — override text fill color",
        ),
        (
            "align",
            r#"string — text alignment: "left", "center", "right", "justify""#,
        ),
        ("italic", "#true / #false — override italic rendering"),
        (
            "space-before",
            "(px) or other dimension — extra space above the block",
        ),
        (
            "space-after",
            "(px) or other dimension — extra space below the block",
        ),
    ];

    const SCOPES: &[(&str, &str)] = &[
        (
            "document",
            "Declared as a direct child of the `document id=… { … }` block. \
          Lowest cascade precedence — applies when neither the page nor the text node \
          declares a matching role.",
        ),
        (
            "page",
            "Declared as a child of a `page id=… { … }` block (alongside `safe-zone` and `fold`). \
          Middle cascade precedence — overrides the document scope for this page's text nodes.",
        ),
        (
            "text",
            "Declared as a child of a `text id=… { … }` block (before `span` children). \
          Highest cascade precedence — overrides both document and page scope for this node.",
        ),
    ];

    const CASCADE_NOTE: &str = "Cascade precedence: text > page > document. \
        When the same `role` is declared at multiple scopes, the most-specific scope wins \
        property-by-property (fine-grained merging is a later unit; in this unit the whole \
        `BlockStyle` struct is stored per scope and the layout engine merges at consume time). \
        Block decls are consumed ONLY on text nodes with `format=\"markdown\"`; they have no \
        effect on plain-text or non-markdown nodes.";

    // Source syntax that PRODUCES each block role (for agent discoverability).
    // Uses r##"..."## because headings contain '#'.
    const SOURCE_SYNTAX: &[(&str, &str)] = &[
        (
            "h1..h6",
            r##"# H1  ## H2  ### H3  #### H4  ##### H5  ###### H6  (ATX headings)"##,
        ),
        ("p", "blank line between paragraphs"),
        ("blockquote", "> text on its own line"),
        (
            "li",
            "- item  or  * item  or  + item  (unordered);  1. item  (ordered)",
        ),
        (
            "code-block",
            "``` (optional lang)\ncode lines\n```  (fenced; lang after opening fence is optional)",
        ),
        ("hr", "--- or *** or ___ on its own line"),
    ];
    // Inline marks (not block roles, but shown here for completeness since block decls
    // apply to the same format=\"markdown\" nodes).
    const INLINE_SYNTAX: &str =
        "**bold**  *italic*  ~~strike~~  ==highlight==  ++underline++  `code`  [label](url)";

    const V1_LIMITS: &str = "v1 limitation: in a chain flow, code-block backgrounds and --- rules are not drawn \
         and blockquote/list indent is not applied. These render fully only in a single \
         non-chained text box.";

    const EXAMPLE: &str = concat!(
        "document id=\"doc.main\" {\n",
        "  block role=\"h1\" font-size=(token)\"size.h1\" font-weight=(token)\"weight.bold\" space-after=(px)16\n",
        "  block role=\"p\"  space-after=(px)8\n",
        "  page id=\"pg.cover\" w=(px)1280 h=(px)720 {\n",
        "    block role=\"h1\" fill=(token)\"color.accent\"\n",
        "    text id=\"body\" format=\"markdown\" src=\"article.md\" x=(px)80 y=(px)80 w=(px)1120 h=(px)560 {\n",
        "      block role=\"p\" space-after=(px)4\n",
        "    }\n",
        "  }\n",
        "}",
    );

    if json {
        use serde_json::{json, to_string_pretty};
        let roles: Vec<&str> = role_vocab.to_vec();
        let props: Vec<serde_json::Value> = PROPS
            .iter()
            .map(|(name, desc)| json!({ "name": name, "description": desc }))
            .collect();
        let scopes: Vec<serde_json::Value> = SCOPES
            .iter()
            .map(|(name, desc)| json!({ "scope": name, "description": desc }))
            .collect();
        let source_syntax: Vec<serde_json::Value> = SOURCE_SYNTAX
            .iter()
            .map(|(role, syntax)| json!({ "role": role, "source_syntax": syntax }))
            .collect();
        let out = json!({
            "schema": "zenith-schema-v1",
            "surface": "block",
            "role_vocabulary": roles,
            "markdown_source_syntax": source_syntax,
            "markdown_inline_syntax": INLINE_SYNTAX,
            "v1_limitations": V1_LIMITS,
            "properties": props,
            "scopes": scopes,
            "cascade": CASCADE_NOTE,
            "example": EXAMPLE,
        });
        (to_string_pretty(&out).unwrap_or_else(|e| e.to_string()), 0)
    } else {
        let mut text = String::new();
        text.push_str("block role=\"…\" — per-role markdown block style declaration\n");
        text.push_str("\nRole vocabulary and markdown source syntax:\n");
        let col = SOURCE_SYNTAX
            .iter()
            .map(|(r, _)| r.len())
            .max()
            .unwrap_or(0);
        for (role, syntax) in SOURCE_SYNTAX {
            text.push_str(&format!("  {role:<col$}  {syntax}\n", col = col));
        }
        text.push_str(&format!(
            "\nInline marks (format=\"markdown\"):\n  {INLINE_SYNTAX}\n"
        ));
        text.push_str(&format!("\nv1 limitations:\n  {V1_LIMITS}\n"));
        text.push_str("\nProperties (on block role=\"…\" declarations):\n");
        for (name, desc) in PROPS {
            text.push_str(&format!("  {name:<16}  {desc}\n"));
        }
        text.push_str("\nScopes:\n");
        for (scope, desc) in SCOPES {
            text.push_str(&format!("  {scope:<12}  {desc}\n"));
        }
        text.push_str(&format!("\nCascade:\n  {CASCADE_NOTE}\n"));
        text.push_str(&format!("\nExample:\n  {}", EXAMPLE.replace('\n', "\n  ")));
        (text, 0)
    }
}

/// `zenith schema style`: named styles block, recognized keys, and tx ops.
///
/// Returns `(stdout, exit_code)`.
pub fn style(json: bool) -> (String, u8) {
    const SUMMARY: &str = "Named visual styles — reusable fill/type/spacing bundles referenced \
        by `style` / `text-style` on nodes (and shape/connector labels).";

    const PLACEMENT: &str = "Top-level `styles { style id=\"…\" { … } }` block, sibling of \
        `tokens` and `document`.";

    let keys: Vec<&str> = zenith_core::STYLE_RECOGNIZED_KEYS.to_vec();
    let enum_keys: serde_json::Map<String, serde_json::Value> = zenith_core::STYLE_ENUM_KEYS
        .iter()
        .map(|(key, values)| ((*key).to_owned(), serde_json::json!(values)))
        .collect();

    const VALUE_FORMS: &str = "align and v-align take a plain enum value (`align \"center\"`); \
        shadow takes a shadow token; every other key takes a token reference. A node \
        attribute of the same name overrides the style value.";

    const TX: &str = "create_style  — create a style with optional properties map (key → token id)\n\
        set_style_property  — set one recognized key to a token id (an enum value for align / v-align)\n\
        delete_style  — remove a style by id\n\
        (see `zenith schema op create_style`)";

    const EXAMPLE: &str = concat!(
        "styles {\n",
        "  style id=\"cta.label\" {\n",
        "    font-family (token)\"font.body\"\n",
        "    font-size (token)\"size.body\"\n",
        "    fill (token)\"color.primary.content\"\n",
        "  }\n",
        "}\n",
        "shape id=\"cta\" kind=\"process\" text-style=\"cta.label\" { span \"Go\" }\n",
        "\n",
        "# or via tx:\n",
        "# {\"ops\":[{\"op\":\"create_style\",\"id\":\"cta.label\",\"properties\":{\
\"font-family\":\"font.body\",\"font-size\":\"size.body\",\"fill\":\"color.primary.content\"}}]}\n"
    );

    if json {
        let out = serde_json::json!({
            "schema": "zenith-schema-v1",
            "surface": "style",
            "summary": SUMMARY,
            "placement": PLACEMENT,
            "recognized_keys": keys,
            "enum_keys": enum_keys,
            "value_forms": VALUE_FORMS,
            "tx_ops": ["create_style", "set_style_property", "delete_style"],
            "example": EXAMPLE,
        });
        (crate::commands::serialize_pretty(&out), 0)
    } else {
        let mut text = format!("style: {SUMMARY}\n");
        text.push_str(&format!("\nPlacement:\n  {PLACEMENT}\n"));
        text.push_str("\nRecognized property keys (underscore spellings accepted in tx):\n");
        for k in &keys {
            text.push_str(&format!("  - {k}\n"));
        }
        text.push_str(&format!("\nValue forms:\n  {VALUE_FORMS}\n"));
        for (key, values) in zenith_core::STYLE_ENUM_KEYS {
            text.push_str(&format!("  {key}: {}\n", values.join(" | ")));
        }
        text.push_str(&format!("\nTransaction ops:\n  {TX}\n"));
        text.push_str(&format!("\nExample:\n  {}", EXAMPLE.replace('\n', "\n  ")));
        (text, 0)
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_surface_lists_enum_keys_and_shadow() {
        let (out, code) = style(true);
        assert_eq!(code, 0);
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        let keys = v["recognized_keys"].as_array().expect("keys");
        for key in ["align", "v-align", "shadow"] {
            assert!(keys.iter().any(|k| k == key), "{key}");
        }
        assert_eq!(
            v["enum_keys"]["align"],
            serde_json::json!(["start", "center", "end", "justify"])
        );
        assert_eq!(
            v["enum_keys"]["v-align"],
            serde_json::json!(["top", "middle", "bottom"])
        );
        let (text, _) = style(false);
        assert!(text.contains("Value forms:"), "{text}");
        assert!(
            text.contains("align: start | center | end | justify"),
            "{text}"
        );
    }
}
