//! `SetOverride`: set or clear the override of one component child on one
//! instance, plus the component lookups that `FindReplaceText` shares.
//!
//! Addressing: `instance` is the id of an instance node, found like any node
//! id. `target` is the component-local id of the child, as an
//! `override ref="…"` names it. The child must be one the scene's override
//! walk reaches: a descendant through frames and groups.

use zenith_core::{Diagnostic, Document, InstanceNode, Node, Override, PropertyValue, TextSpan};

use super::style::node_fill_mut;
use super::{find_node_any_mut, find_node_any_shared, record_affected};

/// The borrowed fields of a [`crate::op::Op::SetOverride`]. Each field is
/// tri-state: `None` keeps, `Some(None)` removes, `Some(Some(v))` sets.
pub(super) struct OverrideEdit<'a> {
    /// The instance node id.
    pub instance: &'a str,
    /// The component-local id of the child.
    pub target: &'a str,
    /// New text of a text child.
    pub text: Option<Option<&'a str>>,
    /// Color token id for the fill.
    pub fill: Option<Option<&'a str>>,
    /// Visibility on this instance.
    pub visible: Option<Option<bool>>,
}

/// An override of `ref_id` that changes nothing.
pub(super) fn empty_override(ref_id: &str) -> Override {
    Override {
        ref_id: ref_id.to_owned(),
        spans: None,
        fill: None,
        stroke: None,
        stroke_width: None,
        svg_stroke: None,
        svg_fill: None,
        svg_stroke_width: None,
        visible: None,
        source_span: None,
    }
}

/// `true` when an override changes nothing and can go.
fn is_empty(ov: &Override) -> bool {
    let Override {
        ref_id: _,
        spans,
        fill,
        stroke,
        stroke_width,
        svg_stroke,
        svg_fill,
        svg_stroke_width,
        visible,
        source_span: _,
    } = ov;
    spans.is_none()
        && fill.is_none()
        && stroke.is_none()
        && stroke_width.is_none()
        && svg_stroke.is_none()
        && svg_fill.is_none()
        && svg_stroke_width.is_none()
        && visible.is_none()
}

/// The child list of the local component `inst` places, or `None` for an
/// imported (`source`) instance or an undeclared component.
pub(super) fn component_children<'d>(doc: &'d Document, inst: &InstanceNode) -> Option<&'d [Node]> {
    let id = inst.component.as_deref()?;
    doc.components
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.children.as_slice())
}

/// The node with local id `ref_id` below `nodes`, through frames and groups:
/// the nodes the scene's override walk reaches.
pub(super) fn find_override_target<'n>(nodes: &'n [Node], ref_id: &str) -> Option<&'n Node> {
    for node in nodes {
        if node.id() == Some(ref_id) {
            return Some(node);
        }
        let below = match node {
            Node::Frame(f) => find_override_target(&f.children, ref_id),
            Node::Group(g) => find_override_target(&g.children, ref_id),
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
        if below.is_some() {
            return below;
        }
    }
    None
}

/// What the shared pass learns before the instance is edited.
struct Plan {
    /// The new override spans: `Some(None)` removes, `Some(Some(_))` sets.
    spans: Option<Option<Vec<TextSpan>>>,
}

/// One Error with `subject` as the subject id.
fn error(code: &str, message: String, subject: &str) -> Diagnostic {
    Diagnostic::error(code, message, None, Some(subject.to_owned()))
}

/// Check `edit` against the document and work out the new spans. `Err`
/// holds the Error that rejects the op.
fn plan(doc: &Document, edit: &OverrideEdit<'_>) -> Result<Plan, Diagnostic> {
    let OverrideEdit {
        instance,
        target,
        text,
        fill,
        visible,
    } = *edit;
    if text.is_none() && fill.is_none() && visible.is_none() {
        return Err(error(
            "tx.invalid_value",
            "set_override: set at least one of text, fill, or visible".to_owned(),
            instance,
        ));
    }
    let inst = match find_node_any_shared(doc, instance) {
        Some(Node::Instance(inst)) => inst,
        Some(other) => {
            return Err(error(
                "tx.wrong_node_type",
                format!(
                    "set_override: {instance:?} is a {}, not an instance",
                    other.kind_str()
                ),
                instance,
            ));
        }
        None => {
            return Err(error(
                "tx.unknown_node",
                format!("set_override: instance {instance:?} not found in document"),
                instance,
            ));
        }
    };
    // The child the override targets. An imported instance has no local
    // component to check against.
    let child = match component_children(doc, inst) {
        Some(children) => match find_override_target(children, target) {
            Some(child) => Some(child),
            None => {
                return Err(error(
                    "tx.unknown_node",
                    format!(
                        "set_override: ref {target:?} matches no child of component {:?}",
                        inst.component.as_deref().unwrap_or_default()
                    ),
                    instance,
                ));
            }
        },
        None if inst.source.is_some() => None,
        None => {
            return Err(error(
                "tx.unknown_node",
                format!(
                    "set_override: instance {instance:?} names component {:?}, which is not declared",
                    inst.component.as_deref().unwrap_or_default()
                ),
                instance,
            ));
        }
    };
    if let Some(Some(_)) = fill
        && let Some(child) = child
        && node_fill_mut(&mut child.clone()).is_none()
    {
        return Err(error(
            "tx.unsupported_property",
            format!(
                "set_override: fill is not supported on {target:?}, a {} node",
                child.kind_str()
            ),
            instance,
        ));
    }
    let spans = match text {
        None => None,
        Some(None) => Some(None),
        Some(Some(new_text)) => {
            let own = match child {
                Some(Node::Text(t)) => Some(t.spans.as_slice()),
                Some(other) => {
                    return Err(error(
                        "tx.wrong_node_type",
                        format!(
                            "set_override: text needs a text child, but {target:?} is a {}",
                            other.kind_str()
                        ),
                        instance,
                    ));
                }
                None => None,
            };
            // The current override text wins over the component text.
            let current = inst
                .overrides
                .iter()
                .find(|ov| ov.ref_id == target)
                .and_then(|ov| ov.spans.as_deref())
                .or(own);
            Some(Some(vec![one_span(current, new_text)]))
        }
    };
    Ok(Plan { spans })
}

/// One span with `text` and the attributes of the first span of `current`.
/// A data field would replace the new text, so the span drops it.
fn one_span(current: Option<&[TextSpan]>, text: &str) -> TextSpan {
    match current.and_then(<[TextSpan]>::first) {
        Some(first) => TextSpan {
            text: text.to_owned(),
            data_ref: None,
            data_format: None,
            ..first.clone()
        },
        None => TextSpan {
            text: text.to_owned(),
            fill: None,
            font_weight: None,
            font_features: None,
            font_alternates: None,
            letter_spacing: None,
            italic: None,
            underline: None,
            strikethrough: None,
            vertical_align: None,
            footnote_ref: None,
            data_ref: None,
            data_format: None,
            highlight: None,
            code: None,
            link: None,
        },
    }
}

pub(super) fn apply_set_override(
    edit: &OverrideEdit<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let plan = match plan(doc, edit) {
        Ok(plan) => plan,
        Err(diag) => {
            diagnostics.push(diag);
            return;
        }
    };
    let Some(Node::Instance(inst)) = find_node_any_mut(doc, edit.instance) else {
        return;
    };
    let index = match inst
        .overrides
        .iter()
        .position(|ov| ov.ref_id == edit.target)
    {
        Some(index) => index,
        None => {
            inst.overrides.push(empty_override(edit.target));
            inst.overrides.len() - 1
        }
    };
    let Some(ov) = inst.overrides.get_mut(index) else {
        return;
    };
    if let Some(spans) = plan.spans {
        ov.spans = spans;
    }
    if let Some(fill) = edit.fill {
        ov.fill = fill.map(|token| PropertyValue::TokenRef(token.to_owned()));
    }
    if let Some(visible) = edit.visible {
        ov.visible = visible;
    }
    if is_empty(ov) {
        inst.overrides.remove(index);
    }
    record_affected(edit.instance, affected);
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    const SRC: &str = r##"zenith version=1 {
  project id="proj" name="T"
  tokens format="zenith-token-v1" { }
  styles { }
  components {
    component id="card" {
      rect id="card.bg" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      text id="card.title" x=(px)0 y=(px)0 w=(px)100 h=(px)20 { span "Title" italic=#true }
      line id="card.rule" x1=(px)0 y1=(px)0 x2=(px)10 y2=(px)0
    }
  }
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
      instance id="c1" component="card" {
        override ref="card.title" { span "Old" }
      }
      rect id="plain" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}"##;

    fn run(edit: &OverrideEdit<'_>) -> (Document, Vec<Diagnostic>, Vec<String>) {
        let mut doc = KdlAdapter.parse(SRC.as_bytes()).expect("fixture parses");
        let mut diags = Vec::new();
        let mut affected = Vec::new();
        apply_set_override(edit, &mut doc, &mut diags, &mut affected);
        (doc, diags, affected)
    }

    fn edit<'a>(target: &'a str) -> OverrideEdit<'a> {
        OverrideEdit {
            instance: "c1",
            target,
            text: None,
            fill: None,
            visible: None,
        }
    }

    fn overrides(doc: &Document) -> &[Override] {
        match find_node_any_shared(doc, "c1") {
            Some(Node::Instance(inst)) => &inst.overrides,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn text_replaces_the_override_spans() {
        let (doc, diags, affected) = run(&OverrideEdit {
            text: Some(Some("New")),
            ..edit("card.title")
        });
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(affected, ["c1"]);
        let spans = overrides(&doc)[0].spans.as_ref().expect("spans");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "New");
    }

    #[test]
    fn a_new_text_override_keeps_the_component_span_attributes() {
        let mut doc = KdlAdapter.parse(SRC.as_bytes()).expect("fixture parses");
        if let Some(Node::Instance(inst)) = find_node_any_mut(&mut doc, "c1") {
            inst.overrides.clear();
        }
        let mut diags = Vec::new();
        let mut affected = Vec::new();
        let e = OverrideEdit {
            text: Some(Some("Fresh")),
            ..edit("card.title")
        };
        apply_set_override(&e, &mut doc, &mut diags, &mut affected);
        assert!(diags.is_empty(), "{diags:?}");
        let spans = overrides(&doc)[0].spans.as_ref().expect("spans");
        assert_eq!(spans[0].text, "Fresh");
        assert_eq!(spans[0].italic, Some(true));
    }

    #[test]
    fn fill_and_visible_add_an_override_and_null_removes_it() {
        let (doc, diags, _) = run(&OverrideEdit {
            fill: Some(Some("color.accent")),
            visible: Some(Some(false)),
            ..edit("card.bg")
        });
        assert!(diags.is_empty(), "{diags:?}");
        let ov = overrides(&doc)
            .iter()
            .find(|o| o.ref_id == "card.bg")
            .expect("override");
        assert_eq!(
            ov.fill,
            Some(PropertyValue::TokenRef("color.accent".into()))
        );
        assert_eq!(ov.visible, Some(false));

        let (doc, diags, _) = run(&OverrideEdit {
            text: Some(None),
            ..edit("card.title")
        });
        assert!(diags.is_empty(), "{diags:?}");
        assert!(overrides(&doc).is_empty(), "an empty override goes");
    }

    #[test]
    fn errors_name_the_cause() {
        let code = |e: OverrideEdit<'_>| run(&e).1[0].code.clone();
        assert_eq!(code(edit("card.title")), "tx.invalid_value");
        assert_eq!(
            code(OverrideEdit {
                visible: Some(Some(true)),
                ..edit("card.nope")
            }),
            "tx.unknown_node"
        );
        assert_eq!(
            code(OverrideEdit {
                text: Some(Some("x")),
                ..edit("card.bg")
            }),
            "tx.wrong_node_type"
        );
        assert_eq!(
            code(OverrideEdit {
                fill: Some(Some("color.a")),
                ..edit("card.rule")
            }),
            "tx.unsupported_property"
        );
        assert_eq!(
            code(OverrideEdit {
                instance: "plain",
                visible: Some(Some(true)),
                ..edit("card.bg")
            }),
            "tx.wrong_node_type"
        );
        assert_eq!(
            code(OverrideEdit {
                instance: "zz",
                visible: Some(Some(true)),
                ..edit("card.bg")
            }),
            "tx.unknown_node"
        );
    }
}
