//! `token_ref`, shared by the test binaries that build `{token}` references.

use zenith_core::PropertyValue;

pub fn token_ref(id: &str) -> PropertyValue {
    PropertyValue::TokenRef(id.to_owned())
}
