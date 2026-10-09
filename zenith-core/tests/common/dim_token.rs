//! `dim_token`, shared by the `validate_geometry` and `validate_shapes` binaries.

use zenith_core::{Dimension, Token, TokenLiteral, TokenType, TokenValue, Unit};

pub fn dim_token(id: &str) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Dimension,
        value: TokenValue::Literal(TokenLiteral::Dimension(Dimension {
            value: 12.0,
            unit: Unit::Px,
        })),
        set: None,
        source_span: None,
    }
}
