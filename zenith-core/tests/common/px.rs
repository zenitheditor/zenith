//! `px`, a pixel `Dimension` builder shared by the `validate_*`, `block_style`,
//! and `format_nodes` test binaries.

use zenith_core::{Dimension, Unit};

pub fn px(v: f64) -> Dimension {
    Dimension {
        value: v,
        unit: Unit::Px,
    }
}
