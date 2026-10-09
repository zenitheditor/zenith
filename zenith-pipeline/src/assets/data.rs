//! Data-context loading from JSON or CSV files for `zenith render --data`.
//!
//! [`load_data_context`] reads a JSON object/array or CSV file and returns a
//! [`DataContext`] populated with flat string scalar fields AND named array
//! columns. JSON nested objects are flattened to dot-paths (`revenue.total`);
//! JSON arrays become named columns in `arrays`. CSV takes the first data row
//! for scalar `fields` and ALL rows as per-column `arrays`.

use std::collections::BTreeMap;
use std::path::Path;

use zenith_core::DataContext;

use crate::io::SourceFs;

// ── Error type ─────────────────────────────────────────────────────────────

/// Error produced while loading a data context file.
#[derive(Debug)]
pub struct DataInputError {
    /// Human-readable description of the failure.
    pub message: String,
}

impl DataInputError {
    fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for DataInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

// ── Public entry point ─────────────────────────────────────────────────────

/// Load a [`DataContext`] from `path`, read through `fs`.
///
/// The file format is inferred from the extension:
/// - `.json` — a JSON **object** (used directly) or a JSON **array** (first
///   element must be an object; empty array or non-object first element →
///   error). Nested objects are flattened to dot-path keys
///   (`{"a":{"b":1}}` → `"a.b" => "1"`). Scalar values: strings are used
///   as-is; numbers and booleans are converted via `to_string`; `null` →
///   empty string. Arrays nested *inside* a data object are skipped.
/// - `.csv` — header row gives field names; the **first data row** supplies
///   values. No data rows → error.
/// - Any other extension → error.
///
/// Returns `Err(DataInputError)` on any I/O, parse, or shape failure.
///
/// # Errors
///
/// See above: any read, parse, or shape failure.
pub fn load_data_context(fs: &dyn SourceFs, path: &Path) -> Result<DataContext, DataInputError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "json" => load_from_json(fs, path),
        "csv" => load_from_csv(fs, path),
        other => Err(DataInputError::new(format!(
            "--data: unsupported file extension '.{other}'; expected .json or .csv"
        ))),
    }
}

// ── JSON loader ────────────────────────────────────────────────────────────

fn load_from_json(fs: &dyn SourceFs, path: &Path) -> Result<DataContext, DataInputError> {
    let bytes = fs.read(path).map_err(|e| {
        DataInputError::new(format!("--data: cannot read '{}': {}", path.display(), e))
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        DataInputError::new(format!(
            "--data: '{}' is not valid UTF-8: {}",
            path.display(),
            e
        ))
    })?;
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| {
        DataInputError::new(format!(
            "--data: '{}' is not valid JSON: {}",
            path.display(),
            e
        ))
    })?;

    // Accept a top-level object or a top-level array (use first element).
    let obj = match value {
        serde_json::Value::Object(map) => map,
        serde_json::Value::Array(arr) => {
            let first = arr.into_iter().next().ok_or_else(|| {
                DataInputError::new(format!(
                    "--data: '{}' is an empty JSON array; expected a non-empty array or object",
                    path.display()
                ))
            })?;
            match first {
                serde_json::Value::Object(map) => map,
                other => {
                    return Err(DataInputError::new(format!(
                        "--data: first element of '{}' is {} not an object",
                        path.display(),
                        json_kind_name(&other)
                    )));
                }
            }
        }
        other => {
            return Err(DataInputError::new(format!(
                "--data: '{}' contains {} not a JSON object or array",
                path.display(),
                json_kind_name(&other)
            )));
        }
    };

    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut arrays: BTreeMap<String, Vec<String>> = BTreeMap::new();
    flatten_object(&obj, String::new(), &mut fields, &mut arrays);
    Ok(DataContext { fields, arrays })
}

/// Recursively flatten a JSON object into dot-path scalar keys and array columns.
///
/// Scalar values (string, number, bool, null) are written into `out_fields`
/// under their dot-path key. Array values whose elements are all scalars are
/// collected into `out_arrays` under the same dot-path key; nested-object or
/// nested-array elements within an array are silently skipped (the rest of the
/// array still populates the column).
fn flatten_object(
    obj: &serde_json::Map<String, serde_json::Value>,
    prefix: String,
    out_fields: &mut BTreeMap<String, String>,
    out_arrays: &mut BTreeMap<String, Vec<String>>,
) {
    for (key, val) in obj {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match val {
            serde_json::Value::Object(inner) => {
                flatten_object(inner, path, out_fields, out_arrays);
            }
            serde_json::Value::Array(arr) => {
                // Collect scalar elements in order; skip nested objects/arrays.
                let strings: Vec<String> = arr
                    .iter()
                    .filter_map(|e| match e {
                        serde_json::Value::Number(n) => Some(n.to_string()),
                        serde_json::Value::String(s) => Some(s.clone()),
                        serde_json::Value::Bool(b) => Some(b.to_string()),
                        serde_json::Value::Null => Some(String::new()),
                        _ => None,
                    })
                    .collect();
                if !strings.is_empty() {
                    out_arrays.insert(path, strings);
                }
            }
            serde_json::Value::String(s) => {
                out_fields.insert(path, s.clone());
            }
            serde_json::Value::Number(n) => {
                out_fields.insert(path, n.to_string());
            }
            serde_json::Value::Bool(b) => {
                out_fields.insert(path, b.to_string());
            }
            serde_json::Value::Null => {
                out_fields.insert(path, String::new());
            }
        }
    }
}

/// Return a short human-readable type name for error messages.
fn json_kind_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

// ── CSV loader ─────────────────────────────────────────────────────────────

fn load_from_csv(fs: &dyn SourceFs, path: &Path) -> Result<DataContext, DataInputError> {
    let bytes = fs.read(path).map_err(|e| {
        DataInputError::new(format!("--data: cannot read '{}': {}", path.display(), e))
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        DataInputError::new(format!(
            "--data: '{}' is not valid UTF-8: {}",
            path.display(),
            e
        ))
    })?;

    // Flexible: tolerate rows with fewer/more fields than the header; short
    // rows are padded per-column below so a series stays category-aligned.
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| {
            DataInputError::new(format!(
                "--data: CSV header error in '{}': {}",
                path.display(),
                e
            ))
        })?
        .clone();

    // Collect ALL data rows so we can build per-column arrays.
    let mut all_records: Vec<csv::StringRecord> = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|e| {
            DataInputError::new(format!(
                "--data: CSV parse error in '{}': {}",
                path.display(),
                e
            ))
        })?;
        all_records.push(record);
    }

    // Require at least one data row (preserves the existing documented contract).
    if all_records.is_empty() {
        return Err(DataInputError::new(format!(
            "--data: '{}' has a header but no data rows",
            path.display()
        )));
    }

    // `fields`: first data row only (scalar KPI use — existing behaviour unchanged).
    // all_records is non-empty: the is_empty() guard above returned early on empty.
    let fields: BTreeMap<String, String> = all_records
        .first()
        .map(|first_record| {
            headers
                .iter()
                .zip(first_record.iter())
                .map(|(h, v)| (h.to_owned(), v.to_owned()))
                .collect()
        })
        .unwrap_or_default();

    // `arrays`: per-column slices across ALL rows, keyed by header name.
    // Short rows have missing cells filled with an empty string to keep
    // per-series length consistent with the category count.
    let mut arrays: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (col_idx, header) in headers.iter().enumerate() {
        let column: Vec<String> = all_records
            .iter()
            .map(|rec| rec.get(col_idx).unwrap_or("").to_owned())
            .collect();
        arrays.insert(header.to_owned(), column);
    }

    Ok(DataContext { fields, arrays })
}

// ── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemFs;

    /// Collect an array accessor's `&[String]` into borrowed `&str`s for
    /// comparison against `&str` literals.
    fn as_strs(arr: Option<&[String]>) -> Option<Vec<&str>> {
        arr.map(|a| a.iter().map(String::as_str).collect())
    }

    /// Load `content` stored as `data<suffix>` in a memory file set.
    fn load(suffix: &str, content: &[u8]) -> Result<DataContext, DataInputError> {
        let path = format!("data{suffix}");
        let fs = MemFs::new().with(&path, content.to_vec());
        load_data_context(&fs, Path::new(&path))
    }

    #[test]
    fn json_flat_object_fields() {
        let ctx = load(".json", br#"{"name": "Alice", "age": 30, "active": true}"#).unwrap();
        assert_eq!(ctx.get("name"), Some("Alice"));
        assert_eq!(ctx.get("age"), Some("30"));
        assert_eq!(ctx.get("active"), Some("true"));
    }

    #[test]
    fn json_null_becomes_empty_string() {
        let ctx = load(".json", br#"{"x": null}"#).unwrap();
        assert_eq!(ctx.get("x"), Some(""));
    }

    #[test]
    fn json_nested_object_flattens() {
        let ctx = load(
            ".json",
            br#"{"revenue": {"total": 42, "tax": 3.5}, "label": "Q1"}"#,
        )
        .unwrap();
        assert_eq!(ctx.get("revenue.total"), Some("42"));
        assert_eq!(ctx.get("revenue.tax"), Some("3.5"));
        assert_eq!(ctx.get("label"), Some("Q1"));
        // The parent key is not inserted.
        assert_eq!(ctx.get("revenue"), None);
    }

    #[test]
    fn json_nested_array_is_skipped() {
        let ctx = load(".json", br#"{"tags": [1, 2, 3], "val": "ok"}"#).unwrap();
        assert_eq!(ctx.get("val"), Some("ok"));
        assert_eq!(ctx.get("tags"), None);
    }

    #[test]
    fn json_array_first_element_used() {
        let ctx = load(
            ".json",
            br##"[{"color": "#ff0000"}, {"color": "#00ff00"}]"##,
        )
        .unwrap();
        assert_eq!(ctx.get("color"), Some("#ff0000"));
    }

    #[test]
    fn json_empty_array_is_error() {
        let err = load(".json", b"[]").unwrap_err();
        assert!(err.message.contains("empty JSON array"), "{}", err.message);
    }

    #[test]
    fn json_array_non_object_first_element_is_error() {
        let err = load(".json", b"[42]").unwrap_err();
        assert!(err.message.contains("not an object"), "{}", err.message);
    }

    #[test]
    fn json_top_level_scalar_is_error() {
        let err = load(".json", b"\"hello\"").unwrap_err();
        assert!(
            err.message.contains("not a JSON object or array"),
            "{}",
            err.message
        );
    }

    #[test]
    fn csv_header_and_first_row() {
        let ctx = load(".csv", b"name,city\nAlice,Wonderland\nBob,Nowhere").unwrap();
        assert_eq!(ctx.get("name"), Some("Alice"));
        assert_eq!(ctx.get("city"), Some("Wonderland"));
    }

    #[test]
    fn csv_no_data_rows_is_error() {
        let err = load(".csv", b"name,city\n").unwrap_err();
        assert!(err.message.contains("no data rows"), "{}", err.message);
    }

    #[test]
    fn unknown_extension_is_error() {
        let err = load(".toml", b"key = \"val\"").unwrap_err();
        assert!(
            err.message.contains("unsupported file extension"),
            "{}",
            err.message
        );
    }

    #[test]
    fn absent_file_is_a_read_error() {
        let fs = MemFs::new();
        let err = load_data_context(&fs, Path::new("d.json")).unwrap_err();
        assert_eq!(
            err.message,
            "--data: cannot read 'd.json': no such file in the in-memory file set"
        );
    }

    #[test]
    fn json_fields_are_sorted() {
        let ctx = load(".json", br#"{"z": "last", "a": "first", "m": "middle"}"#).unwrap();
        let keys: Vec<&str> = ctx.fields.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["a", "m", "z"]);
    }

    #[test]
    fn json_array_value_populates_arrays() {
        let ctx = load(".json", br#"{"sales": [12, 18, 15]}"#).unwrap();
        assert_eq!(
            as_strs(ctx.get_array("sales")),
            Some(vec!["12", "18", "15"]),
        );
        // The key does not appear in scalar fields.
        assert_eq!(ctx.get("sales"), None);
    }

    #[test]
    fn json_array_with_mixed_scalars() {
        let ctx = load(".json", br#"{"vals": [1, "two", true, null]}"#).unwrap();
        assert_eq!(
            as_strs(ctx.get_array("vals")),
            Some(vec!["1", "two", "true", ""]),
        );
    }

    #[test]
    fn json_empty_array_is_not_inserted() {
        let ctx = load(".json", br#"{"empty": [], "x": "y"}"#).unwrap();
        assert!(ctx.get_array("empty").is_none());
        assert_eq!(ctx.get("x"), Some("y"));
    }

    #[test]
    fn json_scalar_and_array_coexist() {
        let ctx = load(".json", br#"{"name": "Alice", "scores": [10, 20, 30]}"#).unwrap();
        assert_eq!(ctx.get("name"), Some("Alice"));
        assert_eq!(
            as_strs(ctx.get_array("scores")),
            Some(vec!["10", "20", "30"])
        );
    }

    #[test]
    fn csv_all_rows_populate_arrays() {
        let ctx = load(".csv", b"month,revenue\nJan,100\nFeb,200\nMar,150").unwrap();
        // Scalar fields: first row only.
        assert_eq!(ctx.get("month"), Some("Jan"));
        assert_eq!(ctx.get("revenue"), Some("100"));
        // Array columns: all rows.
        assert_eq!(
            as_strs(ctx.get_array("month")),
            Some(vec!["Jan", "Feb", "Mar"]),
        );
        assert_eq!(
            as_strs(ctx.get_array("revenue")),
            Some(vec!["100", "200", "150"]),
        );
    }

    #[test]
    fn csv_short_row_pads_with_empty_string() {
        // The second row lacks the revenue cell.
        let ctx = load(".csv", b"month,revenue\nJan,100\nFeb").unwrap();
        assert_eq!(as_strs(ctx.get_array("revenue")), Some(vec!["100", ""]));
    }
}
