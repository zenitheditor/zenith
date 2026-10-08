//! Numeric preflight for strict PDF reports.

mod commands;
mod images;
mod transform;
mod values;

pub(super) use commands::{check_capture_commands, check_scene};
pub(super) use images::{RasterDimensions, check_image};

#[cfg(test)]
mod tests;
