//! `font_family_token`, shared by the `validate_geometry` and `validate_nodes`
//! binaries.

use zenith_core::{Token, TokenLiteral, TokenType, TokenValue};

pub fn font_family_token(id: &str) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::FontFamily,
        value: TokenValue::Literal(TokenLiteral::String("Inter".to_owned())),
        set: None,
        source_span: None,
    }
}
