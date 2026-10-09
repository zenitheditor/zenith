//! `ping`: report the engine version.

use serde_json::Value;

/// The `ping` result: `{"version": "<crate version>"}`.
pub(crate) fn run() -> Value {
    serde_json::json!({ "version": env!("CARGO_PKG_VERSION") })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_reports_crate_version() {
        assert_eq!(run()["version"], env!("CARGO_PKG_VERSION"));
    }
}
