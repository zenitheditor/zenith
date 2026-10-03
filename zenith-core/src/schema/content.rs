//! Child-content descriptors for node kinds that accept authorable children.

// ── Child content descriptors ─────────────────────────────────────────────────

/// Full content descriptor for a node kind that accepts authorable child content.
///
/// Returned by [`node_content`].
pub struct NodeContentDescriptor {
    /// Short prose description of what the child content represents.
    pub description: &'static str,
    /// A minimal, syntactically correct example of the child content written
    /// inside the parent node's block (without the surrounding node wrapper).
    pub example: &'static str,
}

/// Return the child-content descriptor for the given node kind, or `None` if
/// the kind accepts no authorable child content (e.g. `rect`, `ellipse`, `line`).
///
/// The match here is exhaustive over all authorable node kinds so that adding a
/// new kind forces a deliberate decision about child content at compile time.
/// Kinds with no child content return `None`.
pub fn node_content(kind: &str) -> Option<NodeContentDescriptor> {
    match kind {
        // ── Span-bearing kinds ────────────────────────────────────────────────
        "text" => Some(NodeContentDescriptor {
            description: "One or more `span` children carry the text runs. \
                Each span takes a string argument and optional inline style props: \
                fill, font-weight, font-features, font-alternates, letter-spacing, italic, underline, strikethrough, highlight, \
                code, link, vertical-align, footnote-ref. \
                `font-features` is a comma-separated OpenType feature list such as \
                `liga=0,kern=1,ss01`; bare tags default to value 1. \
                `font-alternates` is a comma-separated alternate list such as \
                `styleset(1),character-variant(2)=3,stylistic`. \
                `letter-spacing` is an optional dimension inserted between adjacent shaped clusters. \
                Node-level `kern-pair \"A\" \"V\" by=(px)-2` children add manual spacing adjustments \
                for matching adjacent shaped clusters; `by` accepts a dimension literal or dimension token. \
                `highlight` is a per-span background color (token ref or raw color string) \
                rendered behind the glyph run like a marker-pen highlight. \
                `code=#true` renders the span in the bundled monospace family with a subtle \
                background, suitable for inline code. \
                `link=\"url\"` renders the span underlined in the default link color (unless \
                `fill` is set); in PDF output the URL becomes a clickable `/Link` annotation \
                over the span. \
                `selectable` (node attribute, default `#true`) controls PDF text extraction: \
                by default the text is emitted as real, selectable / searchable / indexable \
                text (with a ToUnicode map, so copy and search work and links are clickable); \
                `selectable=#false` renders the glyphs as filled outlines instead — visually \
                identical but not selectable, searchable, or extractable. The PNG backend is \
                unaffected. \
                The `format` node attribute (values: `markdown` | `plain`) opts into \
                markdown rendering of the concatenated span text. \
                When `format=\"markdown\"`, the scene compile pass re-parses the span content \
                AFTER data-binding substitution and renders both inline marks and block structure. \
                Inline marks: `**bold**`, `*italic*`, `~~strike~~`, `==highlight==`, \
                `++underline++`, `` `code` ``, `[label](url)`. \
                Block structure (one construct per line/paragraph): \
                `# H1` through `###### H6` (ATX headings), blank line separates paragraphs, \
                `> text` blockquote, `- item` / `* item` / `+ item` unordered list, \
                `1. item` ordered list, ` ``` ` fenced code block (optional lang after opening \
                fence; ends at closing ` ``` `), `---` / `***` / `___` horizontal rule. \
                The block roles produced (h1..h6, p, blockquote, li, code-block, hr) are the \
                same names styled by `block role=\"…\"` declarations (see `zenith schema block`). \
                v1 limitation: in a `chain` flow, code-block backgrounds and `---` rules are \
                not drawn and blockquote/list indent is not applied — these render fully only \
                in a single non-chained text box. \
                Pairs well with a single `data-ref` span to parse external content as markdown \
                without encoding marks in the document. `format=\"plain\"` or absent = literal \
                (byte-identical to today's behavior). \
                The `src` node attribute (`src=\"path/to/file.md\"`) loads the file at the \
                given path (resolved relative to the document's project directory) and uses its \
                UTF-8 contents as the node's text content, replacing any inline `span` children \
                at render time. This keeps the `.zen` file lean for long-form prose. When paired \
                with `format=\"markdown\"`, the loaded text is parsed as markdown by the \
                scene compile pass. A missing or unreadable file emits a `text.src_missing` \
                Error diagnostic (same gate as `asset.missing`). The `src` field is retained \
                on the node so a future editor can write edits back to the original file. \
                Overflow (`overflow` attribute) means the glyph ink leaves the box vertically, \
                or one unbreakable line is wider than the box. `clip` (default) clips ink at the box edge and \
                warns `text.overflow`. `visible` paints past the box silently. `fit` keeps the \
                size and errors `text.fit_failed`. `autofit` shrinks the font to the largest \
                integer px in [`font-size-min`, `font-size`] that fits, else errors \
                `text.fit_failed` at the floor. Both diagnostics name the `h`/`w` that fits at \
                the declared size and the font size that fits the box. \
                Threaded text flow (`chain` attribute): all `text` nodes that share the same \
                `chain=\"id\"` value form one ordered chain (document source order, across pages). \
                The FIRST member that carries spans or `src` content is the content source; \
                subsequent members must have EMPTY spans (no `src`, no inline spans) and serve \
                as overflow boxes. Each member needs explicit `x`/`y`/`w`/`h` geometry. Text \
                fills box 1, the remainder flows into box 2, etc., across page boundaries. \
                A chain is how you resolve a `text.overflow` warning for long-form copy: add \
                chained continuation boxes (on the same or new pages) until nothing overflows. \
                Only the first member's font/style drives the whole chain; per-span overrides \
                on the source are honored. \
                A `block role=\"…\"` declaration may appear BEFORE span children to set per-role \
                markdown block style at this text node's scope (highest cascade precedence: \
                text > page > document). Block decls affect only nodes with `format=\"markdown\"` \
                and have no effect on plain-text nodes (see `zenith schema block`).",
            example: concat!(
                "block role=\"h1\" font-size=(token)\"size.h1\" font-weight=(token)\"weight.bold\"\n",
                "kern-pair \"A\" \"V\" by=(px)-2\n",
                "span \"Hello \"\n",
                "span \"world\" font-weight=(token)\"weight.bold\" italic=#true",
            ),
        }),
        "shape" => Some(NodeContentDescriptor {
            description: "Optional `span` children form a text label rendered centered inside the \
                shape. Use h-align/v-align on the shape node to adjust alignment. \
                Omit the block entirely for an unlabelled shape.",
            example: "span \"Approve\"",
        }),
        "footnote" => Some(NodeContentDescriptor {
            description: "One or more `span` children carry the footnote body text, \
                using the same span model as `text`.",
            example: "span \"See also Chapter 3.\"",
        }),

        // ── Vertex-bearing kinds ──────────────────────────────────────────────
        "polygon" => Some(NodeContentDescriptor {
            description: "Two or more `point` children define the closed vertex list in order. \
                Each point carries `x` and `y` as px-literal dimensions.",
            example: concat!(
                "point x=(px)0 y=(px)0\n",
                "point x=(px)100 y=(px)0\n",
                "point x=(px)50 y=(px)86",
            ),
        }),
        "polyline" => Some(NodeContentDescriptor {
            description: "Two or more `point` children define the open vertex list in order. \
                Each point carries `x` and `y` as px-literal dimensions.",
            example: concat!(
                "point x=(px)0 y=(px)0\n",
                "point x=(px)100 y=(px)50\n",
                "point x=(px)200 y=(px)0",
            ),
        }),
        "path" => Some(NodeContentDescriptor {
            description: "A path is either a legacy direct-anchor contour or a compound path made \
                from one or more `subpath` children; do not mix both forms. Direct paths use two or \
                more `anchor` children for an open Bezier path, or three or more when `closed=#true`. \
                Compound paths put the same anchor rules inside each `subpath`, with optional \
                per-subpath `closed=#true`; parent `closed` is not accepted with subpaths. Each anchor \
                carries required `x` and `y` dimensions, optional authoring intent `kind` \
                (corner|smooth|symmetric), plus optional paired `in-x`/`in-y` and `out-x`/`out-y` \
                handles.",
            example: concat!(
                "subpath closed=#true {\n",
                "    anchor x=(px)0 y=(px)0 out-x=(px)20 out-y=(px)0\n",
                "    anchor x=(px)80 y=(px)0 kind=\"smooth\" in-x=(px)60 in-y=(px)0 out-x=(px)100 out-y=(px)40\n",
                "    anchor x=(px)80 y=(px)80 in-x=(px)100 in-y=(px)40\n",
                "}\n",
                "subpath closed=#true {\n",
                "    anchor x=(px)24 y=(px)24\n",
                "    anchor x=(px)56 y=(px)24\n",
                "    anchor x=(px)56 y=(px)56\n",
                "}",
            ),
        }),

        // ── Structured container kinds ────────────────────────────────────────
        "table" => Some(NodeContentDescriptor {
            description: "Optional `column` children (each with `width=(px)N`) declare column \
                widths; then `row` children each containing `cell` children. \
                Each cell accepts colspan, rowspan, fill, border, h-align, v-align, \
                and arbitrary renderable child nodes for cell content. \
                Cell text auto-places: the cell sizes and positions its text into the content box \
                (padding-inset), wraps to the column width, and aligns via the cell/table \
                `h-align` (start|center|end) and `v-align` (top|middle|bottom). \
                Omit `x`/`y`/`w`/`h` on cell text; set them only to override the auto layout. \
                The table itself requires its own `x`/`y`/`w`/`h`.",
            example: concat!(
                "column width=(px)120\n",
                "column width=(px)80\n",
                "row {\n",
                "    cell { text { span \"Name\" } }\n",
                "    cell { text { span \"Score\" } }\n",
                "}",
            ),
        }),

        // ── Generic container kinds ───────────────────────────────────────────
        "frame" => Some(NodeContentDescriptor {
            description: "Arbitrary renderable child nodes (any node kind). \
                layout=\"absolute\" (default) keeps child x/y and clips to the frame box. \
                layout=\"row\" / \"column\" stack children left to right / top to bottom \
                inside the padded box, `gap` apart, with justify (start|center|end|\
                space-between), align (start|center|end|stretch), and wrap=#true \
                (lines `wrap-gap` apart). A child sizes each axis by its w/h: a fixed \
                dimension, \"hug\" (its content), or \"fill\" (a share of the free \
                space), clamped by min-*/max-*. An omitted main-axis size hugs; an \
                omitted cross-axis size fills under align=\"stretch\". A layout frame \
                without w/h hugs its children. position=\"absolute\" children leave the \
                flow and place x/y from the frame's top-left. layout=\"grid\" tiles \
                children into `columns` × `rows` cells. `gap` and `padding*` take a px \
                literal or a dimension token. Stacking frames do not clip unless \
                clip=#true. Frame fill/stroke/stroke-width/radius paint the box under \
                the children; a clipping frame with a radius clips to the rounded box. \
                A hugging `image` takes its asset's pixel size (one fixed axis keeps the \
                aspect). A hugging `instance` takes its component's content bounds; a \
                fixed or fill size fits the component into the box (`fit`, default \
                contain). A layout frame placed by `anchor` is measured first, then \
                anchored at its measured size.",
            example: "rect id=\"bg\" x=(px)0 y=(px)0 w=(px)400 h=(px)300 fill=(token)\"color.bg\"",
        }),
        "group" => Some(NodeContentDescriptor {
            description: "Arbitrary renderable child nodes (any node kind). \
                May also include `protected-region id=... x=... y=... w=... h=...` \
                and `editable-param id=...` metadata children.",
            example: "rect id=\"box\" x=(px)0 y=(px)0 w=(px)100 h=(px)100",
        }),

        // ── Series-bearing kind ───────────────────────────────────────────────
        "chart" => Some(NodeContentDescriptor {
            description: "Optional `categories` child carries the X-axis category labels as \
                positional string arguments (one per slot; absent = derive index labels at render). \
                Optional `label-colors` child carries per-slice value-label colors as positional \
                PropertyValue arguments (e.g. `(token)\"color.x\"`; one per category in order; \
                absent = use the chart value-color or the white on-fill default). \
                Optional `slice-colors` child carries per-slice FILL colors for pie/donut as \
                positional PropertyValue arguments (e.g. `(token)\"color.x\"`; one per category \
                in order; absent = use the palette). \
                Zero or more `series` children carry the numeric data. \
                Each series node takes its f64 data values as positional arguments \
                and optional named props: label, color (token ref), label-color (token ref), data-ref. \
                A `data-ref=\"field\"` binds the whole series to a numeric ARRAY field supplied at \
                render via `--data` — JSON `{\"field\":[120,185,143]}` or a CSV column named `field` \
                (one number per category). This is render-time binding, distinct from `zenith merge`, \
                which substitutes per-row scalar text/image via `role=\"data.<column>\"` and does not \
                vary chart series per row. \
                Emit `categories` then `label-colors` then `slice-colors` before any `series` children.",
            example: concat!(
                "categories \"Q1\" \"Q2\" \"Q3\" \"Q4\"\n",
                "label-colors (token)\"color.c1\" (token)\"color.c2\" (token)\"color.c3\" (token)\"color.c4\"\n",
                "slice-colors (token)\"color.s1\" (token)\"color.s2\" (token)\"color.s3\" (token)\"color.s4\"\n",
                "series label=\"Revenue\" color=(token)\"color.primary\" label-color=(token)\"color.lbl\" 120.0 200.0 150.0 310.0\n",
                "series label=\"Costs\" color=(token)\"color.secondary\" 80.0 90.0 100.0 120.0",
            ),
        }),
        "light" | "mesh" => None,

        // ── Motif-bearing kind ────────────────────────────────────────────────
        "pattern" => Some(NodeContentDescriptor {
            description: "Exactly one required child node — the motif — which is the template \
                node that gets tiled. Any authorable node kind is valid as the motif.",
            example: "rect id=\"dot\" x=(px)0 y=(px)0 w=(px)8 h=(px)8 fill=(token)\"color.accent\"",
        }),

        // ── Override-bearing kind ─────────────────────────────────────────────
        "instance" => Some(NodeContentDescriptor {
            description: "Zero or more `override` children apply per-node property overrides \
                to descendants of the referenced component. Each override targets a node by \
                `ref=\"id\"` and accepts fill, stroke, stroke-width, SVG image style fields, \
                visible, and optional `span` children to replace text content.",
            example: concat!(
                "override ref=\"headline\" fill=(token)\"color.alt\" stroke=(token)\"color.line\" {\n",
                "    span \"New headline text\"\n",
                "}",
            ),
        }),

        // ── Verbatim-content kind ─────────────────────────────────────────────
        "code" => Some(NodeContentDescriptor {
            description: "A single `content` child carries the verbatim source string as its \
                first positional argument. Newlines and tabs are expressed as \\n and \\t \
                escape sequences in the string literal. Optional node-level `kern-pair` children \
                before `content` add manual spacing adjustments; `by` accepts a dimension literal \
                or dimension token.",
            example: "kern-pair \"=\" \">\" by=(token)\"size.kern.tight\"\ncontent \"fn main() {\\n    println!(\\\"hello\\\");\\n}\"",
        }),

        // ── Connector label ───────────────────────────────────────────────────
        "connector" => Some(NodeContentDescriptor {
            description: "Optional `span` children form a text label rendered at the \
                connector's geometric midpoint (the mid-point of the routed polyline). \
                Use `text-style` on the connector node to apply a style ref to the label. \
                Nudge the label with `label-offset-x` / `label-offset-y` (px) off the midpoint. \
                Connectors can target semantic ports with `from=\"node#port\"`; declare \
                page/component ports in a sibling `ports { port node=\"...\" id=\"...\" \
                anchor=\"...\" }` metadata block. Omit the block entirely (or author no \
                `span` children) for an unlabelled connector — the render output is \
                byte-identical when no spans are present.",
            example: "span \"Yes\"",
        }),

        // ── No authorable child content ───────────────────────────────────────
        "rect" | "ellipse" | "line" | "image" | "field" | "toc" => None,

        // Any unrecognised kind also has no content description.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every authorable node kind that is expected to have child content must
    /// return `Some` from `node_content`, and the example must be non-empty.
    ///
    /// Kinds confirmed to carry authorable child content (parser-verified):
    /// text, shape, footnote, polygon, polyline, path, table, frame, group, pattern, chart, instance,
    /// code, connector (optional span label).
    #[test]
    fn node_content_returns_some_for_content_bearing_kinds() {
        let content_kinds = &[
            "text",
            "shape",
            "footnote",
            "polygon",
            "polyline",
            "path",
            "table",
            "frame",
            "group",
            "pattern",
            "chart",
            "instance",
            "code",
            "connector",
        ];
        for &kind in content_kinds {
            let desc = node_content(kind);
            assert!(
                desc.is_some(),
                "node_content(\"{kind}\") returned None — expected Some for a content-bearing kind",
            );
            let d = desc.unwrap();
            assert!(
                !d.description.is_empty(),
                "node_content(\"{kind}\").description is empty",
            );
            assert!(
                !d.example.is_empty(),
                "node_content(\"{kind}\").example is empty",
            );
        }
    }

    /// Kinds with no authorable child content must return `None` from `node_content`.
    #[test]
    fn node_content_returns_none_for_no_content_kinds() {
        let no_content_kinds = &["rect", "ellipse", "line", "image", "field", "toc"];
        for &kind in no_content_kinds {
            assert!(
                node_content(kind).is_none(),
                "node_content(\"{kind}\") returned Some — expected None for a leaf-only kind",
            );
        }
    }
}
