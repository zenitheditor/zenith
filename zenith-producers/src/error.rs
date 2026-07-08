use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProduceError {
    UnsupportedRequest {
        producer: &'static str,
        request: &'static str,
    },
    ZpxBake(String),
    SvgNative(String),
}

impl ProduceError {
    pub(crate) const fn unsupported_request(producer: &'static str, request: &'static str) -> Self {
        Self::UnsupportedRequest { producer, request }
    }
}

impl fmt::Display for ProduceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedRequest { producer, request } => {
                write!(f, "{producer} cannot produce from {request}")
            }
            Self::ZpxBake(message) => write!(f, "ZPX bake failed: {message}"),
            Self::SvgNative(message) => write!(f, "SVG native conversion failed: {message}"),
        }
    }
}

impl Error for ProduceError {}

impl From<zenith_zpx::ZpxError> for ProduceError {
    fn from(value: zenith_zpx::ZpxError) -> Self {
        Self::ZpxBake(value.to_string())
    }
}
