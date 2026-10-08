//! Typed MCP render arguments are checked before filesystem access.

use super::req_str;
use crate::commands::render::check_render_scale;
use serde_json::Value;

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
