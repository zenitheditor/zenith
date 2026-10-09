//! Host-facing warnings that are not diagnostics: the [`WarningSink`] trait.

use std::cell::RefCell;

/// Receives warnings that do not belong in a diagnostic list (for example, a
/// project font asset that fails to parse and is skipped).
pub trait WarningSink {
    /// Report one warning. `message` has no `warning:` prefix.
    fn warn(&self, message: &str);
}

/// Drops every warning.
#[derive(Debug, Clone, Copy, Default)]
pub struct IgnoreWarnings;

impl WarningSink for IgnoreWarnings {
    fn warn(&self, _message: &str) {}
}

/// Keeps every warning in report order.
#[derive(Debug, Default)]
pub struct CollectWarnings {
    messages: RefCell<Vec<String>>,
}

impl CollectWarnings {
    /// An empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The warnings reported so far, in order.
    #[must_use]
    pub fn take(&self) -> Vec<String> {
        self.messages.take()
    }
}

impl WarningSink for CollectWarnings {
    fn warn(&self, message: &str) {
        self.messages.borrow_mut().push(message.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collector_keeps_order() {
        let sink = CollectWarnings::new();
        sink.warn("a");
        sink.warn("b");
        IgnoreWarnings.warn("c");
        assert_eq!(sink.take(), vec!["a".to_owned(), "b".to_owned()]);
        assert!(sink.take().is_empty());
    }
}
