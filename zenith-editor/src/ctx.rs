//! [`Ctx`]: one command run. It owns the working session, counts the
//! engine work the command does, and keeps the last parse so a text is
//! parsed once per call.

use std::rc::Rc;

use serde::Serialize;
use sha2::{Digest, Sha256};
use zenith_core::{Diagnostic, Document, FontMissLog, KdlAdapter, KdlSource};
use zenith_pipeline::{PageRect, PageView, Validation, ViewOptions, validate_parsed, view_page};
use zenith_tx::{Transaction, TxResult, run_transaction};

use crate::env::Env;
use crate::error::EditorError;
use crate::fonts::missing_face_notices;
use crate::session::Session;
use crate::wire::DiagnosticOut;

/// Engine work one command did, counted per call. Deterministic: use it to
/// compare commands instead of wall-clock time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Work {
    /// Document parses. A call parses each text once: later uses of the
    /// same text (validation, render, outline, a later batch step) reuse
    /// it.
    pub parses: u32,
    /// Full validate runs (config, parse, validate, assets, and a compile of
    /// every page without raster).
    pub validations: u32,
    /// `run_transaction` calls.
    pub tx_runs: u32,
    /// Source patcher calls.
    pub patches: u32,
    /// Single-page compiles.
    pub compiles: u32,
    /// Page rasters.
    pub rasters: u32,
}

impl std::ops::AddAssign for Work {
    fn add_assign(&mut self, other: Work) {
        self.parses += other.parses;
        self.validations += other.validations;
        self.tx_runs += other.tx_runs;
        self.patches += other.patches;
        self.compiles += other.compiles;
        self.rasters += other.rasters;
    }
}

/// A rendered page a command produced, kept out of the JSON result.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedImage {
    /// The encoded PNG bytes.
    pub png: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The 1-based page.
    pub page: usize,
    /// The page window the PNG covers, for a viewport render. `None` for a
    /// whole page.
    pub region: Option<ImageRegion>,
}

/// The page window a viewport render covers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageRegion {
    /// Left edge in device pixels of the page at `scale`.
    pub x: u32,
    /// Top edge in device pixels of the page at `scale`.
    pub y: u32,
    /// Device pixels per page pixel.
    pub scale: f64,
    /// Page width in device pixels at `scale`.
    pub device_width: u32,
    /// Page height in device pixels at `scale`.
    pub device_height: u32,
    /// Page width in page pixels (the scene's media box, bleed included).
    pub page_width: f64,
    /// Page height in page pixels.
    pub page_height: f64,
}

impl RenderedImage {
    /// The lowercase hex SHA-256 of the PNG bytes.
    #[must_use]
    pub fn sha256(&self) -> String {
        hex_sha256(&self.png)
    }
}

/// The lowercase hex SHA-256 of `bytes`.
#[must_use]
pub fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A validation of one text, with the dropped-font notices.
pub(crate) struct Checked {
    pub(crate) validation: Validation,
    pub(crate) notices: Vec<DiagnosticOut>,
}

impl Checked {
    /// `true` when the text parsed and no diagnostic is an Error.
    pub(crate) fn valid(&self) -> bool {
        self.validation.exit_code == 0
    }

    /// The diagnostics located over `src`, then the font notices.
    pub(crate) fn diagnostics(&self, src: &str) -> Vec<DiagnosticOut> {
        let mut out = DiagnosticOut::all(&self.validation.diagnostics, src);
        out.extend(self.notices.iter().cloned());
        out
    }
}

/// The text render commands use and whether it is stale.
pub(crate) struct Display {
    pub(crate) doc: Rc<Document>,
    pub(crate) text: String,
    pub(crate) stale: bool,
}

/// One command run.
pub(crate) struct Ctx<'e, 'a> {
    pub(crate) env: &'e Env<'a>,
    /// The command id, for messages and offers.
    pub(crate) command: &'e str,
    /// The working session. Discarded when the command fails.
    pub(crate) session: Session,
    pub(crate) work: Work,
    pub(crate) image: Option<RenderedImage>,
    /// The last text parsed and its parse.
    pub(crate) parsed: ParseCache,
}

/// The last text a call parsed and its parse (the document, or the
/// `parse.error` diagnostic). A parse depends on the text alone, so a hit
/// equals a fresh parse.
#[derive(Default)]
pub(crate) struct ParseCache(Option<(String, Result<Rc<Document>, Diagnostic>)>);

impl<'e, 'a> Ctx<'e, 'a> {
    pub(crate) fn new(env: &'e Env<'a>, command: &'e str, session: Session) -> Self {
        Self {
            env,
            command,
            session,
            work: Work::default(),
            image: None,
            parsed: ParseCache::default(),
        }
    }

    /// Parse `text`, or reuse the parse when `text` is the last text parsed.
    pub(crate) fn parse(&mut self, text: &str) -> Result<Rc<Document>, Diagnostic> {
        if let Some((cached, parsed)) = &self.parsed.0
            && cached == text
        {
            return parsed.clone();
        }
        self.work.parses += 1;
        let parsed = KdlAdapter
            .parse(text.as_bytes())
            .map(Rc::new)
            .map_err(|e| Diagnostic::error("parse.error", e.message, e.span, None));
        self.parsed.0 = Some((text.to_owned(), parsed.clone()));
        parsed
    }

    /// Run the full validate pipeline on `text`, recording the face
    /// requests the build cannot serve.
    pub(crate) fn validate(&mut self, text: &str) -> Checked {
        self.work.validations += 1;
        let parsed = self.parse(text);
        let log = FontMissLog::new();
        let host = self.env.host.with_font_log(&log);
        let validation = validate_parsed(
            host,
            parsed.as_deref(),
            self.env.project_dir,
            self.env.flags,
        );
        Checked {
            validation,
            notices: missing_face_notices(&log),
        }
    }

    /// The current text, parsed, for an edit. Edits need a valid text: the
    /// transaction engine rejects a document that already has errors.
    pub(crate) fn editable(&mut self) -> Result<Rc<Document>, EditorError> {
        if !self.session.valid {
            return Err(buffer_invalid(self.command));
        }
        let text = self.session.text.clone();
        self.parse(&text).map_err(|d| {
            EditorError::new(
                "editor.buffer_invalid",
                format!(
                    "the text does not parse ({}); fix it before '{}'",
                    d.message, self.command
                ),
            )
        })
    }

    /// The text render commands use: the current text when valid, else the
    /// last valid text (stale).
    pub(crate) fn display(&mut self) -> Result<Display, EditorError> {
        let stale = self.session.stale();
        let text = self
            .session
            .display_text()
            .map(str::to_owned)
            .ok_or_else(|| {
                EditorError::new(
                    "editor.no_valid_render",
                    "no text of this session has been valid yet; fix the error diagnostics \
                     (doc.diagnose lists them) and send buffer.set",
                )
            })?;
        let doc = self.parse(&text).map_err(|d| {
            EditorError::new(
                "editor.no_valid_render",
                format!("the last valid text no longer parses: {}", d.message),
            )
        })?;
        Ok(Display { doc, text, stale })
    }

    /// Compile 0-based `page_index` of `doc` with boxes, and rasterize it
    /// (or its `viewport` window, page px) when `raster` is set.
    pub(crate) fn view(
        &mut self,
        doc: &Document,
        page_index: usize,
        raster: Option<f64>,
        viewport: Option<PageRect>,
    ) -> Result<PageView, EditorError> {
        self.work.compiles += 1;
        if raster.is_some() {
            self.work.rasters += 1;
        }
        let opts = ViewOptions {
            data: self.env.data,
            lint: false,
            raster,
            viewport,
        };
        view_page(self.env.host, doc, self.env.project_dir, page_index, opts)
            .map_err(|e| pipeline_error(&e, ""))
    }

    /// Run `tx` on `doc`.
    pub(crate) fn run_tx(
        &mut self,
        doc: &Document,
        tx: &Transaction,
    ) -> Result<TxResult, EditorError> {
        self.work.tx_runs += 1;
        run_transaction(doc, tx).map_err(|e| {
            EditorError::new(
                "editor.tx_failed",
                format!("{e}; the document could not be formatted, so no edit ran"),
            )
        })
    }
}

/// `editor.buffer_invalid` for `command`.
pub(crate) fn buffer_invalid(command: &str) -> EditorError {
    EditorError::new(
        "editor.buffer_invalid",
        format!(
            "the text has error diagnostics, and '{command}' edits the document; fix the \
             errors (doc.diagnose lists them) and retry"
        ),
    )
}

/// The editor error of a stopped pipeline call: the code of its first
/// error diagnostic, its message, and its diagnostics over `src`.
pub(crate) fn pipeline_error(e: &zenith_pipeline::PipelineError, src: &str) -> EditorError {
    let code = e
        .diagnostics
        .iter()
        .find(|d| d.is_error())
        .map_or("render.failed", |d| d.code.as_str());
    EditorError::new(code, e.message.clone()).with_diagnostics(&e.diagnostics, src)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_lowercase_hex() {
        assert_eq!(
            hex_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
