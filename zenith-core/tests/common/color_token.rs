//! `color_token`, shared by the `validate_*` test binaries that need a color token.

use zenith_core::{Token, TokenLiteral, TokenType, TokenValue};

pub fn color_token(id: &str) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Color,
        value: TokenValue::Literal(TokenLiteral::String("#112233".to_owned())),
        set: None,
        source_span: None,
    }
}
