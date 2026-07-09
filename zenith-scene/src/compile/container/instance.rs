//! `instance` expansion: clone the referenced component subtree, apply
//! overrides, prefix descendant ids, and delegate to [`compile_group`] for the
//! translation + opacity cascade. Also hosts the id-prefix walk reused by the
//! parent module's master-page projection.

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, Dimension, GroupNode, InstanceNode, Node, Override, PropertyValue, ResolvedToken,
};

use crate::ir::SceneCommand;

use super::super::font_ns::NamespacedFontProvider;
use super::super::imports::{ImportSource, parse_import_source};
use super::super::util::resolve_geometry_px;
use super::super::{NodeCtx, RenderCtx};
use super::group::{compile_group, group_children_bounds};

/// Compile an `instance` node by expanding its referenced component subtree.
///
/// Expansion strategy (per the component/symbol design):
/// 1. Look the component up in `components`; a missing component emits an
///    advisory `scene.unknown_component` and the instance is skipped.
/// 2. CLONE the component's children (never mutate the stored definition),
///    apply each override to the matching LOCAL-id descendant, then PREFIX every
///    descendant id with the instance id (`<inst-id>/<local-id>`) so multiple
///    instances of the same component never produce duplicate ids in the scene.
/// 3. Wrap the prepared children in a synthetic [`GroupNode`] carrying the
///    instance's `x`/`y` origin (as the group translation) and its
///    `opacity`/`visible` cascade, then delegate to [`compile_group`]. This
///    reuses the group translation + opacity-cascade machinery verbatim rather
///    than duplicating it; the instance itself emits no command.
pub(in crate::compile) fn compile_instance(
    instance: &InstanceNode,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    connector_strokes: &mut Vec<usize>,
    ctx: RenderCtx,
) {
    // Entire expansion excluded when visible=false (mirror group/frame).
    if instance.visible == Some(false) {
        return;
    }

    if let Some(source) = &instance.source {
        compile_imported_instance(
            instance,
            source,
            cx,
            commands,
            diagnostics,
            connector_strokes,
            ctx,
        );
        return;
    }

    let Some(component_id) = instance.component.as_deref() else {
        return;
    };

    let Some(component) = cx.components.get(component_id) else {
        diagnostics.push(Diagnostic::advisory(
            "scene.unknown_component",
            format!(
                "instance '{}' references component '{}' which is not declared; \
                 the instance is skipped",
                instance.id, component_id
            ),
            instance.source_span,
            Some(instance.id.clone()),
        ));
        return;
    };

    // Clone the component subtree (the stored definition is never mutated),
    // apply overrides against LOCAL ids, then prefix ids with the instance id.
    let mut children = component.children.clone();
    for ov in &instance.overrides {
        apply_override(&mut children, ov);
    }
    let prefix = format!("{}/", instance.id);
    prefix_ids_in_children(&mut children, &prefix);

    // Build a synthetic group carrying the instance origin + cascade and reuse
    // compile_group's translation/opacity logic. The group's own id is the
    // instance id (it emits no command, so the id is only for self-consistency).
    let synthetic = synthetic_group(instance, children);

    compile_group(
        &synthetic,
        cx,
        commands,
        diagnostics,
        connector_strokes,
        ctx,
    );
}

fn compile_imported_instance(
    instance: &InstanceNode,
    source: &str,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    connector_strokes: &mut Vec<usize>,
    ctx: RenderCtx,
) {
    if !cx.imports.is_enabled() {
        diagnostics.push(Diagnostic::advisory(
            "scene.unsupported_import_source",
            format!(
                "instance '{}' references imported source '{}' which is not yet expanded by scene compile; the instance is skipped",
                instance.id, source
            ),
            instance.source_span,
            Some(instance.id.clone()),
        ));
        return;
    }

    let (import_id, component_id) = match parse_import_source(source) {
        ImportSource::Component {
            import_id,
            component_id,
        } => (import_id, component_id),
        ImportSource::Page { import_id, page_id } => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_import_target",
                format!(
                    "instance '{}' references unsupported import target '{}#page.{}'; the instance is skipped",
                    instance.id, import_id, page_id
                ),
                instance.source_span,
                Some(instance.id.clone()),
            ));
            return;
        }
        ImportSource::UnsupportedTarget { import_id, target } => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_import_target",
                format!(
                    "instance '{}' references unsupported import target '{}#{}'; the instance is skipped",
                    instance.id, import_id, target
                ),
                instance.source_span,
                Some(instance.id.clone()),
            ));
            return;
        }
        ImportSource::Invalid => {
            diagnostics.push(invalid_import_source(instance, source));
            return;
        }
    };

    let Some(imported) = cx.imports.get(import_id) else {
        diagnostics.push(Diagnostic::advisory(
            "scene.unknown_import",
            format!(
                "instance '{}' references import '{}' which is not in the scene import graph; the instance is skipped",
                instance.id, import_id
            ),
            instance.source_span,
            Some(instance.id.clone()),
        ));
        return;
    };

    let Some(component) = imported.components.get(component_id) else {
        diagnostics.push(Diagnostic::advisory(
            "scene.unknown_import_component",
            format!(
                "instance '{}' references component '{}' in import '{}' but it is not declared; the instance is skipped",
                instance.id, component_id, import_id
            ),
            instance.source_span,
            Some(instance.id.clone()),
        ));
        return;
    };

    let mut children = component.children.clone();
    for ov in &instance.overrides {
        let override_value = remap_import_override(ov, cx);
        apply_override(&mut children, &override_value);
    }
    prefix_imported_asset_refs(&mut children, import_id);
    let prefix = format!("{}/", instance.id);
    prefix_ids_in_children(&mut children, &prefix);

    // Route the imported subtree's font resolution through a namespaced wrapper:
    // its text requests plain family names, which resolve to the import's own
    // faces (registered under `"{import_id}/{family}"`) first, then fall back to
    // bundled/host-registered families. Host compilation keeps `cx.fonts`.
    let imported_fonts = NamespacedFontProvider::new(cx.fonts, import_id);
    let imported_cx = NodeCtx {
        resolved: &imported.resolved,
        style_map: &imported.style_map,
        components: &imported.components,
        imports: cx.imports,
        fonts: &imported_fonts,
        engine: cx.engine,
        chains: cx.chains,
        flows: cx.flows,
        anchors: cx.anchors,
        field_ctx: cx.field_ctx,
        md_blocks: cx.md_blocks,
        page_block_styles: &[],
        doc_block_styles: &imported.document.body.block_styles,
    };

    // Resolve the instance `w`/`h`/`fit` against the HOST token scope. When both
    // dimensions resolve to a positive px box, the imported subtree is scaled to
    // fit that box (mirroring the page-source fit path); otherwise the current
    // translate-only path is preserved exactly (byte-identical for no-w/h).
    match fit_outcome(
        instance,
        &children,
        &imported.resolved,
        cx,
        ctx,
        diagnostics,
    ) {
        FitOutcome::Skip => {}
        FitOutcome::Transform { sx, sy, tx, ty } => {
            let mut synthetic = synthetic_group(instance, children);
            // The transform carries all positioning; the synthetic group compiles
            // its children at the local origin (0,0).
            synthetic.x = None;
            synthetic.y = None;
            let local_ctx = RenderCtx {
                opacity: ctx.opacity,
                dx: 0.0,
                dy: 0.0,
                baseline_grid: ctx.baseline_grid,
            };
            if is_identity_transform(sx, sy, tx, ty) {
                compile_group(
                    &synthetic,
                    imported_cx,
                    commands,
                    diagnostics,
                    connector_strokes,
                    local_ctx,
                );
            } else {
                commands.push(SceneCommand::PushScaleTranslate { sx, sy, tx, ty });
                compile_group(
                    &synthetic,
                    imported_cx,
                    commands,
                    diagnostics,
                    connector_strokes,
                    local_ctx,
                );
                commands.push(SceneCommand::PopTransform);
            }
        }
        FitOutcome::Translate => {
            let synthetic = synthetic_group(instance, children);
            compile_group(
                &synthetic,
                imported_cx,
                commands,
                diagnostics,
                connector_strokes,
                ctx,
            );
        }
    }
}

/// The scaling decision for an imported instance carrying `w`/`h`/`fit`.
enum FitOutcome {
    /// No (positive) `w`/`h` box, or the source bounds are unresolvable: keep the
    /// translate-only path (byte-identical to the pre-fit behavior).
    Translate,
    /// A resolved scale + translate transform to apply around the subtree.
    Transform { sx: f64, sy: f64, tx: f64, ty: f64 },
    /// An unknown `fit` value was diagnosed; the instance is skipped entirely.
    Skip,
}

/// Resolve the imported-instance fit transform.
///
/// `w`/`h`/`x`/`y` resolve against the HOST token scope (`cx.resolved`); the
/// source bounds resolve against the imported scope (`imported_resolved`), since
/// the children carry imported-scope geometry. Mirrors `page_source::fit_transform`.
fn fit_outcome(
    instance: &InstanceNode,
    children: &[Node],
    imported_resolved: &BTreeMap<String, ResolvedToken>,
    cx: NodeCtx,
    ctx: RenderCtx,
    diagnostics: &mut Vec<Diagnostic>,
) -> FitOutcome {
    let (Some(w), Some(h)) = (
        instance_dim_px(instance.w.as_ref(), cx.resolved),
        instance_dim_px(instance.h.as_ref(), cx.resolved),
    ) else {
        return FitOutcome::Translate;
    };
    if !(w > 0.0 && h > 0.0) {
        return FitOutcome::Translate;
    }

    let Some((smin_x, smin_y, sw, sh)) =
        group_children_bounds(children, 0.0, 0.0, imported_resolved)
    else {
        return FitOutcome::Translate;
    };
    if !(sw > 0.0 && sh > 0.0) {
        return FitOutcome::Translate;
    }

    let x_px = instance_dim_px(instance.x.as_ref(), cx.resolved).unwrap_or(0.0);
    let y_px = instance_dim_px(instance.y.as_ref(), cx.resolved).unwrap_or(0.0);
    let dev_dx = ctx.dx + x_px;
    let dev_dy = ctx.dy + y_px;

    // Default fit is "contain" whenever a w/h box is present.
    let (sx, sy, ox, oy) = match instance.fit.as_deref().unwrap_or("contain") {
        "contain" => {
            let scale = (w / sw).min(h / sh);
            (scale, scale, (w - sw * scale) / 2.0, (h - sh * scale) / 2.0)
        }
        "fill" => (w / sw, h / sh, 0.0, 0.0),
        "none" => (1.0, 1.0, 0.0, 0.0),
        other => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_import_target",
                format!(
                    "instance '{}' uses unsupported fit '{}'; the instance is skipped",
                    instance.id, other
                ),
                instance.source_span,
                Some(instance.id.clone()),
            ));
            return FitOutcome::Skip;
        }
    };

    FitOutcome::Transform {
        sx,
        sy,
        tx: dev_dx + ox - sx * smin_x,
        ty: dev_dy + oy - sy * smin_y,
    }
}

/// Resolve an instance geometry [`Dimension`] to px against `resolved`, reusing
/// the shared [`resolve_geometry_px`] path (a raw dimension resolves directly).
fn instance_dim_px(
    dim: Option<&Dimension>,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<f64> {
    let prop = dim.cloned().map(PropertyValue::Dimension);
    resolve_geometry_px(prop.as_ref(), resolved)
}

/// Whether a `PushScaleTranslate` would be the identity (no-op).
fn is_identity_transform(sx: f64, sy: f64, tx: f64, ty: f64) -> bool {
    sx == 1.0 && sy == 1.0 && tx == 0.0 && ty == 0.0
}

fn invalid_import_source(instance: &InstanceNode, source: &str) -> Diagnostic {
    Diagnostic::advisory(
        "scene.invalid_import_source",
        format!(
            "instance '{}' has malformed import source '{}'; expected import-id#component.component-id and the instance is skipped",
            instance.id, source
        ),
        instance.source_span,
        Some(instance.id.clone()),
    )
}

fn prefix_imported_asset_refs(nodes: &mut [Node], import_id: &str) {
    for node in nodes {
        match node {
            Node::Image(image) => {
                image.asset = format!("{}/{}", import_id, image.asset);
            }
            Node::Frame(frame) => prefix_imported_asset_refs(&mut frame.children, import_id),
            Node::Group(group) => prefix_imported_asset_refs(&mut group.children, import_id),
            Node::Table(table) => {
                for row in &mut table.rows {
                    for cell in &mut row.cells {
                        prefix_imported_asset_refs(&mut cell.children, import_id);
                    }
                }
            }
            Node::Unknown(unknown) => prefix_imported_asset_refs(&mut unknown.children, import_id),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => {}
        }
    }
}

fn remap_import_override(ov: &Override, cx: NodeCtx) -> Override {
    let mut remapped = ov.clone();
    remap_color_override(&mut remapped.fill, cx);
    remap_color_override(&mut remapped.stroke, cx);
    remap_color_override(&mut remapped.svg_stroke, cx);
    remap_color_override(&mut remapped.svg_fill, cx);
    remapped
}

fn remap_color_override(prop: &mut Option<PropertyValue>, cx: NodeCtx) {
    if let Some(PropertyValue::TokenRef(token_id)) = prop
        && let Some(token) = cx.resolved.get(token_id)
        && let Some(hex) = token.value.as_color_hex()
    {
        *prop = Some(PropertyValue::Literal(hex.to_owned()));
    }
}

fn synthetic_group(instance: &InstanceNode, children: Vec<Node>) -> GroupNode {
    GroupNode {
        id: instance.id.clone(),
        name: instance.name.clone(),
        role: instance.role.clone(),
        x: instance.x.clone().map(PropertyValue::Dimension),
        y: instance.y.clone().map(PropertyValue::Dimension),
        w: None,
        h: None,
        opacity: instance.opacity,
        visible: instance.visible,
        locked: instance.locked,
        rotate: None,
        blend_mode: None,
        shadow: None,
        filter: None,
        mask: None,
        blur: None,
        style: None,
        semantic_role: None,
        intensity: None,
        layer_priority: None,
        symmetry_count: None,
        symmetry_cx: None,
        symmetry_cy: None,
        symmetry_start_angle: None,
        symmetry_mode: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        children,
        protected_regions: Vec::new(),
        editable_param_ids: Vec::new(),
        source_span: instance.source_span,
        unknown_props: BTreeMap::new(),
    }
}

/// Apply a single [`Override`] to the first descendant in `children` (descending
/// into `group`/`frame`/`instance` containers) whose LOCAL id equals
/// `ov.ref_id`. Mutates a CLONE — callers pass the cloned component subtree.
///
/// Supported payload: replace `spans` (text targets), `fill`, native stroke
/// style, image `svg-*`, and `visible`.
/// An override targeting a kind without the relevant field is a no-op for that
/// field (e.g. `spans` on a rect). An unmatched ref is silently ignored here;
/// the validator already warns via `component.unknown_override_target`.
fn apply_override(children: &mut [Node], ov: &Override) -> bool {
    for child in children.iter_mut() {
        if node_local_id(child) == Some(ov.ref_id.as_str()) {
            apply_override_to_node(child, ov);
            return true;
        }
        let grandchildren = match child {
            Node::Frame(f) => Some(&mut f.children),
            Node::Group(g) => Some(&mut g.children),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Table(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => None,
        };
        if let Some(gc) = grandchildren
            && apply_override(gc, ov)
        {
            return true;
        }
    }
    false
}

/// Merge an override's supported fields onto a single matched node.
fn apply_override_to_node(node: &mut Node, ov: &Override) {
    // spans → only a text node carries spans.
    if let Some(spans) = &ov.spans
        && let Node::Text(t) = node
    {
        t.spans = spans.clone();
    }
    // fill → the kinds that carry a fill property.
    if let Some(fill) = &ov.fill {
        set_node_fill(node, fill.clone());
    }
    if let Some(stroke) = &ov.stroke {
        set_node_stroke(node, stroke.clone());
    }
    if let Some(stroke_width) = &ov.stroke_width {
        set_node_stroke_width(node, stroke_width.clone());
    }
    if ov.svg_stroke.is_some() || ov.svg_fill.is_some() || ov.svg_stroke_width.is_some() {
        set_node_svg_style(node, ov);
    }
    // visible → every id-bearing renderable kind carries a visible flag.
    if let Some(v) = ov.visible {
        set_node_visible(node, v);
    }
}

fn set_node_svg_style(node: &mut Node, ov: &Override) {
    let Node::Image(image) = node else {
        return;
    };
    if let Some(stroke) = &ov.svg_stroke {
        image.svg_stroke = Some(stroke.clone());
    }
    if let Some(fill) = &ov.svg_fill {
        image.svg_fill = Some(fill.clone());
    }
    if let Some(stroke_width) = &ov.svg_stroke_width {
        image.svg_stroke_width = Some(stroke_width.clone());
    }
}

/// Set the `stroke` of a node kind that carries one; a no-op for kinds without
/// a stroke property.
fn set_node_stroke(node: &mut Node, stroke: PropertyValue) {
    match node {
        Node::Rect(n) => n.stroke = Some(stroke),
        Node::Ellipse(n) => n.stroke = Some(stroke),
        Node::Line(n) => n.stroke = Some(stroke),
        Node::Polygon(n) => n.stroke = Some(stroke),
        Node::Polyline(n) => n.stroke = Some(stroke),
        Node::Path(n) => n.stroke = Some(stroke),
        Node::Connector(n) => n.stroke = Some(stroke),
        Node::Shape(n) => n.stroke = Some(stroke),
        Node::Mesh(n) => n.stroke = Some(stroke),
        Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Table(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Unknown(_) => {}
    }
}

/// Set the `stroke-width` of a node kind that carries one; a no-op for kinds
/// without a stroke-width property.
fn set_node_stroke_width(node: &mut Node, stroke_width: PropertyValue) {
    match node {
        Node::Rect(n) => n.stroke_width = Some(stroke_width),
        Node::Ellipse(n) => n.stroke_width = Some(stroke_width),
        Node::Line(n) => n.stroke_width = Some(stroke_width),
        Node::Polygon(n) => n.stroke_width = Some(stroke_width),
        Node::Polyline(n) => n.stroke_width = Some(stroke_width),
        Node::Path(n) => n.stroke_width = Some(stroke_width),
        Node::Connector(n) => n.stroke_width = Some(stroke_width),
        Node::Shape(n) => n.stroke_width = Some(stroke_width),
        Node::Mesh(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Table(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Unknown(_) => {}
    }
}

/// Set the `fill` of a node kind that carries one; a no-op for kinds without
/// a fill property.
fn set_node_fill(node: &mut Node, fill: PropertyValue) {
    match node {
        Node::Rect(n) => n.fill = Some(fill),
        Node::Ellipse(n) => n.fill = Some(fill),
        Node::Text(n) => n.fill = Some(fill),
        Node::Code(n) => n.fill = Some(fill),
        Node::Polygon(n) => n.fill = Some(fill),
        Node::Polyline(n) => n.fill = Some(fill),
        Node::Path(n) => n.fill = Some(fill),
        Node::Field(n) => n.fill = Some(fill),
        Node::Toc(n) => n.fill = Some(fill),
        Node::Footnote(n) => n.fill = Some(fill),
        Node::Table(n) => n.fill = Some(fill),
        Node::Shape(n) => n.fill = Some(fill),
        Node::Pattern(n) => n.fill = Some(fill),
        Node::Chart(n) => n.fill = Some(fill),
        Node::Light(_) => {}
        Node::Mesh(n) => n.stroke = Some(fill),
        // A connector is stroke-only; it has no fill to override.
        Node::Line(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Instance(_)
        | Node::Connector(_)
        | Node::Unknown(_) => {}
    }
}

/// Set the `visible` flag of a node kind that carries one.
fn set_node_visible(node: &mut Node, v: bool) {
    match node {
        Node::Rect(n) => n.visible = Some(v),
        Node::Ellipse(n) => n.visible = Some(v),
        Node::Line(n) => n.visible = Some(v),
        Node::Text(n) => n.visible = Some(v),
        Node::Code(n) => n.visible = Some(v),
        Node::Frame(n) => n.visible = Some(v),
        Node::Group(n) => n.visible = Some(v),
        Node::Image(n) => n.visible = Some(v),
        Node::Polygon(n) => n.visible = Some(v),
        Node::Polyline(n) => n.visible = Some(v),
        Node::Path(n) => n.visible = Some(v),
        Node::Instance(n) => n.visible = Some(v),
        Node::Field(n) => n.visible = Some(v),
        Node::Toc(n) => n.visible = Some(v),
        Node::Table(n) => n.visible = Some(v),
        Node::Shape(n) => n.visible = Some(v),
        Node::Connector(n) => n.visible = Some(v),
        Node::Pattern(n) => n.visible = Some(v),
        Node::Chart(n) => n.visible = Some(v),
        Node::Light(n) => n.visible = Some(v),
        Node::Mesh(n) => n.visible = Some(v),
        // A footnote has no `visible` flag; nothing to set.
        Node::Footnote(_) => {}
        Node::Unknown(_) => {}
    }
}

/// The LOCAL id of a node (the id as authored), or `None` for `Unknown`.
fn node_local_id(node: &Node) -> Option<&str> {
    match node {
        Node::Rect(n) => Some(&n.id),
        Node::Ellipse(n) => Some(&n.id),
        Node::Line(n) => Some(&n.id),
        Node::Text(n) => Some(&n.id),
        Node::Code(n) => Some(&n.id),
        Node::Frame(n) => Some(&n.id),
        Node::Group(n) => Some(&n.id),
        Node::Image(n) => Some(&n.id),
        Node::Polygon(n) => Some(&n.id),
        Node::Polyline(n) => Some(&n.id),
        Node::Path(n) => Some(&n.id),
        Node::Instance(n) => Some(&n.id),
        Node::Field(n) => Some(&n.id),
        Node::Toc(n) => Some(&n.id),
        Node::Footnote(n) => Some(&n.id),
        Node::Table(n) => Some(&n.id),
        Node::Shape(n) => Some(&n.id),
        Node::Connector(n) => Some(&n.id),
        Node::Pattern(n) => Some(&n.id),
        Node::Chart(n) => Some(&n.id),
        Node::Light(n) => Some(&n.id),
        Node::Mesh(n) => Some(&n.id),
        Node::Unknown(_) => None,
    }
}

/// Recursively prepend `prefix` to the id of every id-bearing node in
/// `children`, descending into `group`/`frame` containers (and prefixing nested
/// instance ids too). Mirrors the suffix walk used by `duplicate_page` in
/// zenith-tx (an in-order recursion, deterministic, no HashMap), but applied as
/// a PREFIX with the instance id so two instances of one component never collide.
pub(in crate::compile) fn prefix_ids_in_children(children: &mut [Node], prefix: &str) {
    for child in children.iter_mut() {
        prefix_node_id(child, prefix);
        match child {
            Node::Frame(f) => prefix_ids_in_children(&mut f.children, prefix),
            Node::Group(g) => prefix_ids_in_children(&mut g.children, prefix),
            Node::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        prefix_ids_in_children(&mut cell.children, prefix);
                    }
                }
            }
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => {}
        }
    }
}

/// Prepend `prefix` to a single node's id (a no-op for `Unknown`).
fn prefix_node_id(node: &mut Node, prefix: &str) {
    macro_rules! pre {
        ($field:expr) => {{
            $field = format!("{prefix}{}", $field);
        }};
    }
    match node {
        Node::Rect(n) => pre!(n.id),
        Node::Ellipse(n) => pre!(n.id),
        Node::Line(n) => pre!(n.id),
        Node::Text(n) => pre!(n.id),
        Node::Code(n) => pre!(n.id),
        Node::Frame(n) => pre!(n.id),
        Node::Group(n) => pre!(n.id),
        Node::Image(n) => pre!(n.id),
        Node::Polygon(n) => pre!(n.id),
        Node::Polyline(n) => pre!(n.id),
        Node::Path(n) => pre!(n.id),
        Node::Instance(n) => pre!(n.id),
        Node::Field(n) => pre!(n.id),
        Node::Toc(n) => pre!(n.id),
        Node::Footnote(n) => pre!(n.id),
        Node::Table(n) => pre!(n.id),
        Node::Shape(n) => pre!(n.id),
        Node::Connector(n) => pre!(n.id),
        Node::Pattern(n) => pre!(n.id),
        Node::Chart(n) => pre!(n.id),
        Node::Light(n) => pre!(n.id),
        Node::Mesh(n) => pre!(n.id),
        Node::Unknown(_) => {}
    }
}
