//! Parse error types for zenith-core.

use crate::ast::Span;

/// Codes that identify the category of a parse error.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseErrorCode {
    /// The input bytes are not valid UTF-8.
    NotUtf8,
    /// The UTF-8 source is not valid KDL.
    InvalidKdl,
    /// No top-level `zenith` node was found in the document.
    MissingZenithRoot,
    /// A node appeared in a context where it is not expected.
    UnexpectedNode,
    /// A property value could not be parsed into the expected type.
    InvalidPropertyValue,
}

/// A single error emitted by the parse layer.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// Source span of the offending token or node, if available.
    pub span: Option<Span>,
    /// Stable error category code.
    pub code: ParseErrorCode,
    /// Human-readable description of the error.
    pub message: String,
    /// Ancestor context state, set by [`ParseError::within`].
    context: Option<Box<Context>>,
}

/// Where and how ancestor names are written into a [`ParseError`] message.
#[derive(Debug, Clone, PartialEq, Default)]
struct Context {
    /// Descriptions of the enclosing nodes, innermost first.
    chain: Vec<String>,
    /// True once the chain reaches an ancestor that has an id.
    anchored: bool,
    /// Byte range of the text currently inserted into the message.
    inserted: Option<(usize, usize)>,
    /// Byte offset where ancestor text goes. `None` appends to the message end.
    insert_at: Option<usize>,
}

impl ParseError {
    /// Construct a `ParseError` without a source span.
    pub fn spanless(code: ParseErrorCode, message: impl Into<String>) -> Self {
        Self {
            span: None,
            code,
            message: message.into(),
            context: None,
        }
    }

    /// Construct a `ParseError` with an explicit source span.
    pub fn with_span(code: ParseErrorCode, span: Span, message: impl Into<String>) -> Self {
        Self {
            span: Some(span),
            code,
            message: message.into(),
            context: None,
        }
    }

    /// Construct a `ParseError` with an optional source span.
    pub fn at(span: Option<Span>, code: ParseErrorCode, message: impl Into<String>) -> Self {
        Self {
            span,
            code,
            message: message.into(),
            context: None,
        }
    }

    /// Set the span to `fallback` when the error has none.
    #[must_use]
    pub fn or_span(mut self, fallback: Option<Span>) -> Self {
        if self.span.is_none() {
            self.span = fallback;
        }
        self
    }

    /// Construct a `ParseError` for a node that lacks a required property.
    ///
    /// `span` is the node's source span. `kind` is the node name and
    /// `property_kind` is the value type (`string`, `integer`).
    pub fn missing_property(
        span: Option<Span>,
        kind: &str,
        property_kind: &str,
        key: &str,
    ) -> Self {
        let head = format!("node `{kind}`");
        let insert_at = Some(head.len());
        Self {
            span,
            code: ParseErrorCode::InvalidPropertyValue,
            message: format!("{head} is missing required {property_kind} property `{key}`"),
            context: Some(Box::new(Context {
                insert_at,
                ..Context::default()
            })),
        }
    }

    /// Record that this error arose inside `description` (e.g. `table "t"`).
    ///
    /// The description joins the chain of enclosing nodes. `has_id` marks a
    /// description that names an id, which ends the chain: outer calls are
    /// ignored. The message shows the chain as ` in cell of table "t"`.
    #[must_use]
    pub fn within(mut self, description: String, has_id: bool) -> Self {
        let ctx = self.context.get_or_insert_with(Box::default);
        if ctx.anchored {
            return self;
        }
        ctx.chain.push(description);
        ctx.anchored = has_id;
        let text = format!(" in {}", ctx.chain.join(" of "));
        if let Some((start, end)) = ctx.inserted {
            self.message.replace_range(start..end, "");
        }
        let at = ctx.insert_at.unwrap_or(self.message.len());
        let at = at.min(self.message.len());
        self.message.insert_str(at, &text);
        ctx.inserted = Some((at, at + text.len()));
        self
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

/// An error emitted by the format layer.
///
/// In practice, formatting the minimal v0 node set never fails — return `Err`
/// only for genuinely un-serializable states (unreachable in valid input, but
/// the fallible signature is the contract from the format spec).
#[derive(Debug, Clone, PartialEq)]
pub struct FormatError {
    /// Human-readable description of why formatting failed.
    pub message: String,
}

impl FormatError {
    /// Construct a `FormatError` with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FormatError {}
