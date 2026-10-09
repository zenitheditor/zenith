//! `color_token_hex`, shared by the `validate_contrast`,
//! `validate_contrast_effects`, and `validate_styles` binaries.

use zenith_core::{Token, TokenLiteral, TokenType, TokenValue};

/// Build a color token with a specific hex value.
pub fn color_token_hex(id: &str, hex: &str) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Color,
        value: TokenValue::Literal(TokenLiteral::String(hex.to_owned())),
        set: None,
        source_span: None,
    }
}
