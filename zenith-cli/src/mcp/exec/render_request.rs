//! Typed MCP render arguments are checked before filesystem access.

use super::req_str;
use serde_json::Value;
use zenith_pipeline::render::check_render_scale;
use zenith_pipeline::{DuplexMode, ImposeMode, ImposeOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RenderFormat {
    Png,
    Svg,
    Pdf,
    Scene,
}
impl RenderFormat {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
            Self::Scene => "scene",
        }
    }
}

#[derive(Debug)]
pub(super) struct RenderRequest<'a> {
    pub doc: &'a str,
    pub format: RenderFormat,
    pub out: Option<&'a str>,
    pub page: Option<usize>,
    pub locked: bool,
    pub diagnostics: bool,
    pub contact_sheet: bool,
    pub scale: Option<f64>,
    pub raster_scale: Option<f64>,
    pub cmyk: Option<bool>,
    pub impose: Option<ImposeOptions>,
}
impl<'a> RenderRequest<'a> {
    pub(super) fn parse(args: &'a Value) -> Result<Self, String> {
        let doc = req_str(args, "doc")?;
        let format = match req_str(args, "format")? {
            "png" => RenderFormat::Png,
            "svg" => RenderFormat::Svg,
            "pdf" => RenderFormat::Pdf,
            "scene" => RenderFormat::Scene,
            value => return Err(invalid("format", value, "Pass png, svg, pdf, or scene")),
        };
        let out = optional_string(args, "out")?;
        let page = match args.get("page") {
            None => None,
            Some(value) => {
                let number = value
                    .as_u64()
                    .filter(|number| *number >= 1)
                    .ok_or_else(|| {
                        invalid(
                            "page",
                            &value.to_string(),
                            "Pass an integer greater than zero",
                        )
                    })?;
                Some(usize::try_from(number).map_err(|_| {
                    invalid(
                        "page",
                        &value.to_string(),
                        "Pass a page number supported by this platform",
                    )
                })?)
            }
        };
        let locked = optional_bool(args, "locked")?.unwrap_or(false);
        let diagnostics = optional_bool(args, "diagnostics")?.unwrap_or(false);
        let contact_sheet = optional_bool(args, "contact_sheet")?;
        let scale = optional_scale(args, "scale")?;
        let raster_scale = optional_scale(args, "raster_scale")?;
        let colorspace = optional_string(args, "colorspace")?;
        let print = optional_bool(args, "print")?.unwrap_or(false);
        let impose_raw = optional_string(args, "impose")?;
        if (scale.is_some() || contact_sheet.is_some()) && format != RenderFormat::Png {
            return Err(invalid(
                "scale/contact_sheet",
                format.as_str(),
                "Set format to png",
            ));
        }
        if raster_scale.is_some() && !matches!(format, RenderFormat::Svg | RenderFormat::Pdf) {
            return Err(invalid(
                "raster_scale",
                format.as_str(),
                "Set format to svg or pdf",
            ));
        }
        if (colorspace.is_some() || print || impose_raw.is_some()) && format != RenderFormat::Pdf {
            return Err(invalid(
                "colorspace/print/impose",
                format.as_str(),
                "Set format to pdf",
            ));
        }
        let cmyk = match colorspace {
            Some("srgb") if print => {
                return Err(invalid(
                    "print",
                    "colorspace=srgb",
                    "Omit print or set colorspace to cmyk",
                ));
            }
            Some("srgb") => Some(false),
            Some("cmyk") => Some(true),
            Some(value) => return Err(invalid("colorspace", value, "Pass srgb or cmyk")),
            None if print => Some(true),
            None => None,
        };
        let duplex_raw = optional_string(args, "duplex")?;
        let creep_mm = match args.get("creep_mm") {
            None => None,
            Some(value) => {
                let n = value
                    .as_f64()
                    .filter(|n| n.is_finite() && *n >= 0.0)
                    .ok_or_else(|| invalid("creep_mm", &value.to_string(), "Pass a number >= 0"))?;
                Some(n)
            }
        };
        let signature = match args.get("signature") {
            None => None,
            Some(value) => {
                let n = value
                    .as_u64()
                    .filter(|n| *n >= 4 && *n % 4 == 0)
                    .ok_or_else(|| {
                        invalid(
                            "signature",
                            &value.to_string(),
                            "Pass a positive multiple of 4",
                        )
                    })?;
                Some(usize::try_from(n).map_err(|_| {
                    invalid("signature", &value.to_string(), "Pass a supported integer")
                })?)
            }
        };
        let marks = optional_bool(args, "marks")?.unwrap_or(false);
        let grid_raw = optional_string(args, "grid")?;
        if impose_raw.is_none()
            && (duplex_raw.is_some()
                || creep_mm.is_some()
                || signature.is_some()
                || grid_raw.is_some()
                || marks)
        {
            return Err(invalid(
                "duplex/creep_mm/signature/grid/marks",
                "no impose",
                "Pass impose when using prepress sheet options",
            ));
        }
        let impose = match impose_raw {
            None => None,
            Some(_) if page.is_some() => {
                return Err(invalid("impose", "page", "Omit page when using impose"));
            }
            Some(mode_str) => {
                let mode = match mode_str {
                    "booklet" | "bifold" | "saddle" => ImposeMode::Booklet,
                    "trifold" | "letterfold" => ImposeMode::Trifold,
                    "zfold" | "accordion" => ImposeMode::Zfold,
                    "spread" | "2up" => ImposeMode::Spread,
                    "grid" | "nup" | "cut-and-stack" | "stack" => {
                        let (cols, rows) = match grid_raw {
                            Some(raw) => zenith_pipeline::render::parse_grid_spec(raw)
                                .map_err(|msg| invalid("grid", raw, &msg))?,
                            None => zenith_pipeline::render::DEFAULT_GRID,
                        };
                        if matches!(mode_str, "grid" | "nup") {
                            ImposeMode::Grid { cols, rows }
                        } else {
                            ImposeMode::CutAndStack { cols, rows }
                        }
                    }
                    value => {
                        return Err(invalid(
                            "impose",
                            value,
                            "Pass booklet, trifold, zfold, spread, grid, or cut-and-stack",
                        ));
                    }
                };
                if grid_raw.is_some()
                    && !matches!(
                        mode,
                        ImposeMode::Grid { .. } | ImposeMode::CutAndStack { .. }
                    )
                {
                    return Err(invalid(
                        "grid",
                        mode_str,
                        "Use impose=grid or impose=cut-and-stack with grid",
                    ));
                }
                let creep = creep_mm.unwrap_or(0.0);
                if creep > 0.0 && mode != ImposeMode::Booklet {
                    return Err(invalid("creep_mm", mode_str, "Set impose to booklet"));
                }
                if signature.is_some() && mode != ImposeMode::Booklet {
                    return Err(invalid("signature", mode_str, "Set impose to booklet"));
                }
                let duplex = match duplex_raw {
                    None | Some("short-edge") => DuplexMode::ShortEdge,
                    Some("long-edge") => DuplexMode::LongEdge,
                    Some(value) => {
                        return Err(invalid("duplex", value, "Pass short-edge or long-edge"));
                    }
                };
                Some(ImposeOptions {
                    mode,
                    duplex,
                    creep_mm: creep,
                    signature,
                    marks,
                })
            }
        };
        let raster_scale = raster_scale.or_else(|| print.then_some(300.0 / 96.0));
        Ok(Self {
            doc,
            format,
            out,
            page,
            locked,
            diagnostics,
            contact_sheet: contact_sheet.unwrap_or(false),
            scale,
            raster_scale,
            cmyk,
            impose,
        })
    }
}

fn optional_string<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    args.get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| invalid(key, &value.to_string(), "Pass a string"))
        })
        .transpose()
}
fn optional_bool(args: &Value, key: &str) -> Result<Option<bool>, String> {
    args.get(key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| invalid(key, &value.to_string(), "Pass true or false"))
        })
        .transpose()
}
fn optional_scale(args: &Value, key: &str) -> Result<Option<f64>, String> {
    args.get(key)
        .map(|value| {
            let number = value.as_f64().ok_or_else(|| {
                invalid(
                    key,
                    &value.to_string(),
                    "Pass a number greater than zero and at most 4",
                )
            })?;
            check_render_scale(number, &value.to_string())
                .map_err(|message| format!("error[cli.invalid_argument]: {key}: {message}"))
        })
        .transpose()
}
fn invalid(key: &str, value: &str, action: &str) -> String {
    format!("error[cli.invalid_argument]: invalid {key} '{value}'. {action}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn omitted_fields_keep_defaults_and_unknown_fields_remain_accepted() {
        let args = json!({"doc":"doc.zen","format":"pdf","future":true});
        let request = RenderRequest::parse(&args).unwrap();
        assert_eq!(request.format, RenderFormat::Pdf);
        assert_eq!(request.page, None);
        assert_eq!(request.out, None);
        assert_eq!(request.scale, None);
        assert_eq!(request.raster_scale, None);
        assert!(!request.locked && !request.diagnostics && !request.contact_sheet);
    }

    #[test]
    fn page_conversion_checks_the_native_usize_range() {
        let args = json!({"doc":"doc.zen","format":"png","page":u64::MAX});
        let result = RenderRequest::parse(&args);
        match usize::try_from(u64::MAX) {
            Ok(page) => assert_eq!(result.unwrap().page, Some(page)),
            Err(_) => assert!(result.is_err()),
        }
    }

    #[test]
    fn explicit_false_contact_sheet_is_still_png_only() {
        assert!(
            RenderRequest::parse(&json!({"doc":"doc.zen","format":"svg","contact_sheet":false}))
                .is_err()
        );
        assert!(
            !RenderRequest::parse(&json!({"doc":"doc.zen","format":"png","contact_sheet":false}))
                .unwrap()
                .contact_sheet
        );
    }

    #[test]
    fn malformed_optional_values_return_argument_errors() {
        for (key, values) in [
            (
                "page",
                vec![
                    json!(0),
                    json!(-1),
                    json!(1.5),
                    json!("2"),
                    Value::Null,
                    json!(true),
                ],
            ),
            ("out", vec![json!(1), Value::Null, json!(true)]),
            ("locked", vec![json!(0), json!("false"), Value::Null]),
            ("diagnostics", vec![json!(0), json!("true"), Value::Null]),
            ("contact_sheet", vec![json!(0), json!("true"), Value::Null]),
            (
                "scale",
                vec![
                    json!(0),
                    json!(-1),
                    json!(4.01),
                    json!("NaN"),
                    Value::Null,
                    json!(true),
                ],
            ),
        ] {
            for value in values {
                let mut args = json!({"doc":"doc.zen","format":"png"});
                args[key] = value;
                assert!(RenderRequest::parse(&args).is_err(), "{args}");
            }
        }
        for value in [
            Value::Null,
            json!(0),
            json!(4.01),
            json!("inf"),
            json!(false),
        ] {
            assert!(
                RenderRequest::parse(&json!({"doc":"doc.zen","format":"svg","raster_scale":value}))
                    .is_err()
            );
        }
    }
}
