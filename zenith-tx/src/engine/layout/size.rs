//! `set_geometry` `w` / `h`: a px number, or the `hug` / `fill` keyword that
//! lives on the node's layout item.

use zenith_core::{Diagnostic, Node, SizeKeyword};

use crate::op::SizeInput;

/// A checked `w` / `h` value of `set_geometry`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::engine) enum SizeArg {
    /// A size in px; clears any size keyword on that axis.
    Px(f64),
    /// A size keyword; clears the px size on that axis.
    Keyword(SizeKeyword),
    /// A `null` input: removes both the px size and the keyword on that axis.
    Remove,
}

/// Check one `w` / `h` input: `None` (absent), `Some(None)` (`null`, remove),
/// or `Some(Some(v))`. `Err(())` after pushing `tx.invalid_value` for a
/// keyword other than `hug` / `fill`.
pub(in crate::engine) fn parse_size_arg(
    input: Option<Option<&SizeInput>>,
    axis: &str,
    node_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Option<SizeArg>, ()> {
    let Some(input) = input else {
        return Ok(None);
    };
    match input {
        None => Ok(Some(SizeArg::Remove)),
        Some(SizeInput::Px(v)) => Ok(Some(SizeArg::Px(*v))),
        Some(SizeInput::Keyword(k)) => match SizeKeyword::from_attr(k) {
            Some(kw) => Ok(Some(SizeArg::Keyword(kw))),
            None => {
                diagnostics.push(Diagnostic::error(
                    "tx.invalid_value",
                    format!(
                        "set_geometry: {axis} {k:?} on node {node_id:?} is not a size; \
                         pass a px number, \"hug\", or \"fill\""
                    ),
                    None,
                    Some(node_id.to_owned()),
                ));
                Err(())
            }
        },
    }
}

/// The keyword a size write leaves on the layout item: `Some(Some(k))` sets
/// it, `Some(None)` clears it (a px write or a removal), `None` leaves it.
fn keyword_write(arg: Option<SizeArg>) -> Option<Option<SizeKeyword>> {
    match arg? {
        SizeArg::Px(_) | SizeArg::Remove => Some(None),
        SizeArg::Keyword(k) => Some(Some(k)),
    }
}

/// Write the `w` / `h` keyword side of a size change onto `node`'s layout
/// item. A px size clears the keyword so the node never carries both.
/// Returns `false` when a keyword targets a node without a layout item.
pub(in crate::engine) fn write_size_keywords(
    node: &mut Node,
    w: Option<SizeArg>,
    h: Option<SizeArg>,
) -> bool {
    let (w, h) = (keyword_write(w), keyword_write(h));
    if w.is_none() && h.is_none() {
        return true;
    }
    let wants_keyword = matches!(w, Some(Some(_))) || matches!(h, Some(Some(_)));
    let Some(item) = node.layout_item_mut() else {
        return !wants_keyword;
    };
    if let Some(k) = w {
        item.w_keyword = k;
    }
    if let Some(k) = h {
        item.h_keyword = k;
    }
    true
}
