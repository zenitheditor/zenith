//! Per-kind checks for the "special" leaf nodes that were already extracted as
//! helpers: `polygon`, `polyline`, `path`, `instance`, `field`, `toc`, and `footnote`.
//! None of these recurse into laid-out children at this site.
//!
//! Submodules: `paths` (polygon/polyline/path), `instance`, `field`, `refs`
//! (toc/footnote).

mod field;
mod instance;
mod paths;
mod refs;

pub(in crate::validate::check) use field::check_field;
pub(in crate::validate::check) use instance::check_instance;
pub(in crate::validate::check) use paths::{check_path, check_polygon, check_polyline};
pub(in crate::validate::check) use refs::{check_footnote, check_toc};
