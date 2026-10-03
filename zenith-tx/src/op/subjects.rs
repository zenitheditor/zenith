//! [`Op::position_preserving_subjects`]: the node ids an op promises to keep
//! at their page position.

use zenith_core::{Document, Node};

use super::ops::Op;
use crate::engine::find_node_any_shared;

impl Op {
    /// The ids whose page box this op promises to keep, read from the
    /// document `before` the op runs.
    ///
    /// - `Reparent`: the moved node and every id in its subtree.
    /// - `Ungroup`: the group's children.
    /// - `Group`: the grouped ids.
    /// - Every other op: `None`.
    ///
    /// A `Reparent` or `Ungroup` whose node is missing from `before` gives
    /// an empty list.
    pub fn position_preserving_subjects(&self, before: &Document) -> Option<Vec<String>> {
        match self {
            Op::Reparent { node, .. } => {
                let mut ids = Vec::new();
                if let Some(found) = find_node_any_shared(before, node) {
                    collect_subtree_ids(found, &mut ids);
                }
                Some(ids)
            }
            Op::Ungroup { group_id } => Some(
                find_node_any_shared(before, group_id)
                    .and_then(Node::children)
                    .map(|children| {
                        children
                            .iter()
                            .filter_map(|c| c.id().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
            Op::Group { node_ids, .. } => Some(node_ids.clone()),
            Op::SetTextAlign { .. }
            | Op::MoveForward { .. }
            | Op::MoveBackward { .. }
            | Op::MoveToFront { .. }
            | Op::MoveToBack { .. }
            | Op::SetFill { .. }
            | Op::SetFillRule { .. }
            | Op::SetStroke { .. }
            | Op::SetStrokeWidth { .. }
            | Op::SetVisible { .. }
            | Op::SetLocked { .. }
            | Op::SetGeometry { .. }
            | Op::SetPoints { .. }
            | Op::SetPathAnchors { .. }
            | Op::SetPathAnchorKind { .. }
            | Op::RemovePathAnchor { .. }
            | Op::MovePathAnchor { .. }
            | Op::MovePathHandle { .. }
            | Op::InsertPathAnchor { .. }
            | Op::InsertPathAnchorAtPoint { .. }
            | Op::SimplifyPathAnchors { .. }
            | Op::TransformPathAnchors { .. }
            | Op::SnapPathAnchors { .. }
            | Op::MakePathSymmetric { .. }
            | Op::PathBoolean { .. }
            | Op::AddNode { .. }
            | Op::AddPath { .. }
            | Op::RemoveNode { .. }
            | Op::SetOpacity { .. }
            | Op::ReplaceText { .. }
            | Op::DuplicateNode { .. }
            | Op::DuplicatePage { .. }
            | Op::AlignNodes { .. }
            | Op::SetTextOverflow { .. }
            | Op::AddPage { .. }
            | Op::DeletePage { .. }
            | Op::ReorderPages { .. }
            | Op::AddAsset { .. }
            | Op::SetAsset { .. }
            | Op::DistributeNodes { .. }
            | Op::CreateToken { .. }
            | Op::UpdateTokenValue { .. }
            | Op::SetStyleProperty { .. }
            | Op::CreateStyle { .. }
            | Op::DeleteStyle { .. }
            | Op::CreateMaster { .. }
            | Op::DeleteMaster { .. }
            | Op::SetPageMaster { .. }
            | Op::SetTextDirection { .. }
            | Op::FindReplaceText { .. }
            | Op::SetPageSize { .. }
            | Op::AlignToEdge { .. }
            | Op::CreateRecipe { .. }
            | Op::UpdateRecipe { .. }
            | Op::DeleteRecipe { .. }
            | Op::SetDefault { .. }
            | Op::RemoveDefault { .. }
            | Op::SetLayout(_)
            | Op::DetachPattern { .. } => None,
        }
    }
}

/// Push the id of `node` and of every node below it, in tree order.
fn collect_subtree_ids(node: &Node, ids: &mut Vec<String>) {
    if let Some(id) = node.id() {
        ids.push(id.to_owned());
    }
    match node {
        Node::Frame(f) => f.children.iter().for_each(|c| collect_subtree_ids(c, ids)),
        Node::Group(g) => g.children.iter().for_each(|c| collect_subtree_ids(c, ids)),
        Node::Unknown(u) => u.children.iter().for_each(|c| collect_subtree_ids(c, ids)),
        Node::Table(t) => t
            .rows
            .iter()
            .flat_map(|r| r.cells.iter())
            .flat_map(|cell| cell.children.iter())
            .for_each(|c| collect_subtree_ids(c, ids)),
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
        | Node::Mesh(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    fn doc(src: &str) -> Document {
        KdlAdapter.parse(src.as_bytes()).expect("fixture parses")
    }

    const SRC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="p" w=(px)400 h=(px)300 {
      group id="g" {
        rect id="a" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
        frame id="f" x=(px)20 y=(px)0 w=(px)50 h=(px)50 {
          rect id="f.inner" x=(px)0 y=(px)0 w=(px)5 h=(px)5 fill=(token)"color.k"
        }
      }
      rect id="b" x=(px)100 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
    }
  }
}"##;

    #[test]
    fn reparent_lists_node_and_subtree() {
        let op = Op::Reparent {
            node: "g".into(),
            new_parent: "p".into(),
            position: Default::default(),
        };
        assert_eq!(
            op.position_preserving_subjects(&doc(SRC)),
            Some(vec![
                "g".to_owned(),
                "a".to_owned(),
                "f".to_owned(),
                "f.inner".to_owned()
            ])
        );
    }

    #[test]
    fn ungroup_lists_former_children() {
        let op = Op::Ungroup {
            group_id: "g".into(),
        };
        assert_eq!(
            op.position_preserving_subjects(&doc(SRC)),
            Some(vec!["a".to_owned(), "f".to_owned()])
        );
    }

    #[test]
    fn group_lists_grouped_ids() {
        let op = Op::Group {
            node_ids: vec!["g".into(), "b".into()],
            group_id: "g2".into(),
        };
        assert_eq!(
            op.position_preserving_subjects(&doc(SRC)),
            Some(vec!["g".to_owned(), "b".to_owned()])
        );
    }

    #[test]
    fn missing_node_gives_empty_list() {
        let op = Op::Reparent {
            node: "nope".into(),
            new_parent: "p".into(),
            position: Default::default(),
        };
        assert_eq!(op.position_preserving_subjects(&doc(SRC)), Some(vec![]));
    }

    #[test]
    fn other_ops_give_none() {
        let op = Op::RemoveNode { node: "b".into() };
        assert_eq!(op.position_preserving_subjects(&doc(SRC)), None);
    }
}
