//! Protected output replacement and destination collision checks.

mod collision;
mod identity;
mod message;
mod replacement;

pub(crate) use collision::{OutputGuard, check_distinct};
pub(crate) use message::write_failure;
pub(crate) use replacement::{check_replaceable, write_bytes};

#[cfg(test)]
mod tests;
