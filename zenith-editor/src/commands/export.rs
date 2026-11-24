//! `doc.export {format, page?, colorspace?, print?}`: export the current
//! document to vector SVG or strict PDF (sRGB or print CMYK), returning the
//! encoded bytes inline so both `zenith edit` and the WASM browser editor can
//! download deterministic vector/print outputs without touching disk.

use serde::Deserialize;
use serde_json::{Value, json};
use zenith_core::FontMissLog;
use zenith_pipeline::RenderOptions;
use zenith_pipeline::render::{render_pdf, render_pdf_pages, render_svg};

use super::common::{page_index, params};
use crate::ctx::{Ctx, pipeline_error};
use crate::error::EditorError;
use crate::fonts::missing_face_notices;
use crate::wire::DiagnosticOut;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportParams {
    format: String,
    #[serde(default)]
    page: Option<usize>,
    #[serde(default)]
    colorspace: Option<String>,
    #[serde(default)]
    print: bool,
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Pure-Rust base64 encoder with zero extra dependencies and no unchecked indexing.
fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk.first().copied().unwrap_or(0);
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let triple = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        let idx = [
            ((triple >> 18) & 0x3f) as usize,
            ((triple >> 12) & 0x3f) as usize,
            ((triple >> 6) & 0x3f) as usize,
            (triple & 0x3f) as usize,
        ];
        if let Some(&c) = B64.get(idx[0]) {
            out.push(char::from(c));
        }
        if let Some(&c) = B64.get(idx[1]) {
            out.push(char::from(c));
        }
        if chunk.len() > 1 {
            if let Some(&c) = B64.get(idx[2]) {
                out.push(char::from(c));
            }
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            if let Some(&c) = B64.get(idx[3]) {
                out.push(char::from(c));
            }
        } else {
            out.push('=');
        }
    }
    out
}

pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: ExportParams = params(ctx, raw)?;
    let display = ctx.display()?;
    let page_count = display.doc.body.pages.len();
    let log = FontMissLog::new();
    let env = ctx.env;
    let host = env.host.with_font_log(&log);
    let mut opts = RenderOptions::new(env.flags)
        .with_data(env.data)
        .with_parsed(Some(&display.doc));

    if p.print {
        opts.cmyk = Some(true);
        opts.raster_scale = 3.125;
    } else if let Some(cs) = p.colorspace.as_deref() {
        opts.cmyk = match cs {
            "cmyk" => Some(true),
            "srgb" => Some(false),
            other => {
                return Err(EditorError::new(
                    "editor.invalid_params",
                    format!("unknown colorspace '{other}'; expected 'srgb' or 'cmyk'"),
                ));
            }
        };
    }

    ctx.work.compiles += 1;
    match p.format.as_str() {
        "svg" => {
            let index = page_index(ctx, p.page, page_count)?;
            let page = index + 1;
            let a = render_svg(host, &display.text, env.project_dir, page, opts)
                .map_err(|e| pipeline_error(&e, &display.text))?;
            let bytes = &a.svg;
            let mut diag_out = DiagnosticOut::all(&a.diagnostics, &display.text);
            diag_out.extend(missing_face_notices(&log));
            Ok(json!({
                "format": "svg",
                "mime": "image/svg+xml",
                "page": a.page,
                "page_count": page_count,
                "width": a.width,
                "height": a.height,
                "bytes": bytes.len(),
                "base64": encode_base64(bytes),
                "stale": display.stale,
                "diagnostics": diag_out,
            }))
        }
        "pdf" => {
            let a = match p.page {
                Some(req) => {
                    let index = page_index(ctx, Some(req), page_count)?;
                    render_pdf(host, &display.text, env.project_dir, index + 1, opts)
                }
                None => render_pdf_pages(host, &display.text, env.project_dir, opts),
            }
            .map_err(|e| pipeline_error(&e, &display.text))?;
            let mut diag_out = DiagnosticOut::all(&a.diagnostics, &display.text);
            diag_out.extend(missing_face_notices(&log));
            Ok(json!({
                "format": "pdf",
                "mime": "application/pdf",
                "page": p.page,
                "page_count": if p.page.is_some() { 1 } else { page_count },
                "bytes": a.pdf.len(),
                "base64": encode_base64(&a.pdf),
                "stale": display.stale,
                "diagnostics": diag_out,
            }))
        }
        other => Err(EditorError::new(
            "editor.invalid_params",
            format!("unknown export format '{other}'; expected 'svg' or 'pdf'"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encodes_rfc4648_vectors() {
        assert_eq!(encode_base64(b""), "");
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
        assert_eq!(encode_base64(b"foo"), "Zm9v");
        assert_eq!(encode_base64(b"foobar"), "Zm9vYmFy");
    }
}
