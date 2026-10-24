//! [`Op::changes_page_order`]: whether an op adds, removes, or moves pages.

use super::ops::Op;

impl Op {
    /// `true` when the op adds, removes, or moves pages, so the page order
    /// after it can differ from the order before it.
    pub fn changes_page_order(&self) -> bool {
        match self {
            Op::AddPage { .. }
            | Op::DeletePage { .. }
            | Op::DuplicatePage { .. }
            | Op::ReorderPages { .. }
            | Op::MovePage { .. }
            | Op::SwapPages { .. } => true,
            Op::SetTextAlign { .. }
            | Op::MoveForward { .. }
            | Op::MoveBackward { .. }
            | Op::MoveToFront { .. }
            | Op::MoveToBack { .. }
            | Op::SetFill { .. }
            | Op::SetFillRule { .. }
            | Op::SetStroke { .. }
            | Op::SetStrokeWidth { .. }
            | Op::SetNodeToken { .. }
            | Op::SetSpanText { .. }
            | Op::SetVisible { .. }
            | Op::SetLocked { .. }
            | Op::SetGeometry { .. }
            | Op::NudgeGeometry { .. }
            | Op::SetAnchor(_)
            | Op::NudgeAnchorGap { .. }
            | Op::DetachAnchor { .. }
            | Op::NudgeLinePoints { .. }
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
            | Op::Group { .. }
            | Op::Ungroup { .. }
            | Op::Reparent { .. }
            | Op::AlignNodes { .. }
            | Op::SetTextOverflow { .. }
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
            | Op::SetOverride { .. }
            | Op::FindReplaceText { .. }
            | Op::SetPageSize { .. }
            | Op::AlignToEdge { .. }
            | Op::CreateRecipe { .. }
            | Op::UpdateRecipe { .. }
            | Op::DeleteRecipe { .. }
            | Op::SetDefault { .. }
            | Op::RemoveDefault { .. }
            | Op::SetLayout(_)
            | Op::DetachPattern { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ops_change_the_order_and_node_ops_do_not() {
        let swap = Op::SwapPages {
            a: "p".into(),
            b: "q".into(),
        };
        assert!(swap.changes_page_order());
        let fill = Op::SetVisible {
            node: "n".into(),
            visible: false,
        };
        assert!(!fill.changes_page_order());
    }
}
