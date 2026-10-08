//! Protected output replacement and destination collision checks.

mod collision;
mod identity;
mod replacement;

pub(crate) use collision::{OutputGuard, check_distinct};
pub(crate) use replacement::write_bytes;

#[cfg(test)]
mod tests;
