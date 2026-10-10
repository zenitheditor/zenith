//! `zenith perceive page`: the pipeline's page signals as JSON or text.
//!
//! The measurement lives in [`zenith_pipeline::perceive`]. This module picks
//! the host-free entry point for the CLI and MCP and formats the result.

use std::path::Path;

use zenith_pipeline::perceive::{
    DiagnosticOutput, Fixed, Metric, PageMetrics, PagePerception, PerceiveArtifact, PerceiveReport,
    perceive_pages, report_json,
};
use zenith_pipeline::render::PageSelection;
use zenith_pipeline::{Host, PipelineError, PolicyFlags};

use super::outcome::{PerceiveCmdErr, PerceiveOutcome};

/// Measure the `selection` of pages of `src` on `host`.
///
/// `project_dir` is the document's directory. It locates project fonts,
/// imports, images, and config, as for `zenith render`.
///
/// # Errors
///
/// The pipeline error: parse (exit 2), validation or compile errors (exit 1),
/// a page out of range, an asset, font, or raster error (exit 2).
pub fn page_report(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    selection: PageSelection,
) -> Result<PerceiveArtifact, PerceiveCmdErr> {
    perceive_pages(host, src, project_dir, selection, &PolicyFlags::default()).map_err(cmd_err)
}

/// `zenith perceive page`: the JSON envelope or the text form on stdout.
///
/// The exit code is 0 whenever the pages measure. A perception diagnostic is
/// a signal, not an error.
///
/// # Errors
///
/// As [`page_report`].
pub fn page(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    selection: PageSelection,
    json: bool,
) -> Result<PerceiveOutcome, PerceiveCmdErr> {
    let artifact = page_report(host, src, project_dir, selection)?;
    let stdout = if json {
        report_json(&artifact.report).map_err(cmd_err)?
    } else {
        human(&artifact.report)
    };
    Ok(PerceiveOutcome {
        stdout,
        exit_code: 0,
    })
}

fn cmd_err(e: PipelineError) -> PerceiveCmdErr {
    PerceiveCmdErr {
        message: e.message,
        exit_code: e.exit_code,
    }
}

/// The text form: one block per page, one line per metric.
fn human(report: &PerceiveReport) -> String {
    let mut lines = vec![format!(
        "perceive page ({}): {}",
        report.schema, report.note
    )];
    for page in &report.pages {
        page_lines(page, &mut lines);
    }
    lines.join("\n")
}

fn page_lines(page: &PagePerception, lines: &mut Vec<String>) {
    lines.push(format!(
        "page {} ({}): {}x{} px",
        page.page, page.id, page.width, page.height
    ));
    let bounds = match &page.ink.bounds {
        Some(b) => format!(
            "{},{} to {},{}",
            num(b.min_x),
            num(b.min_y),
            num(b.max_x),
            num(b.max_y)
        ),
        None => "none".to_owned(),
    };
    lines.push(format!(
        "  ink: {} px differ from {}, bounds {bounds}",
        page.ink.pixel_count, page.ink.background
    ));
    metric_lines(&page.metrics, lines);
}

fn metric_lines(m: &PageMetrics, lines: &mut Vec<String>) {
    push_metric(lines, "histogram", &m.histogram, |h| {
        let bins: Vec<String> = h.bins.iter().map(u64::to_string).collect();
        (
            format!(
                "mean {} min {} max {} bins [{}]",
                num(h.mean),
                num(h.min),
                num(h.max),
                bins.join(" ")
            ),
            &h.diagnostics,
        )
    });
    push_metric(lines, "value_zones", &m.value_zones, |v| {
        let zones: Vec<String> = v
            .zones
            .iter()
            .map(|z| format!("{} {} (mean {})", z.zone, z.count, num(z.mean)))
            .collect();
        (zones.join(", "), &v.diagnostics)
    });
    push_metric(lines, "density_map", &m.density_map, |d| {
        (
            format!(
                "focal {},{} top_70 {} next_20 {} remaining_10 {}",
                num(d.focal_x),
                num(d.focal_y),
                num(d.ratios.top_70),
                num(d.ratios.next_20),
                num(d.ratios.remaining_10)
            ),
            &d.diagnostics,
        )
    });
    push_metric(lines, "edge_map", &m.edge_map, |e| {
        (
            format!(
                "count {} hard {} lost {} max {} mean {}",
                e.count,
                e.hard_count,
                e.lost_count,
                num(e.max),
                num(e.mean)
            ),
            &e.diagnostics,
        )
    });
    push_metric(lines, "optical_balance", &m.optical_balance, |o| {
        (
            format!(
                "dx {} dy {} normalized {},{} offset {}",
                num(o.center_dx),
                num(o.center_dy),
                num(o.normalized_dx),
                num(o.normalized_dy),
                num(o.offset_magnitude)
            ),
            &o.diagnostics,
        )
    });
    push_metric(lines, "clearspace", &m.clearspace, |c| {
        (
            format!(
                "left {} right {} top {} bottom {} minimum {} required {} passes {}",
                num(c.left),
                num(c.right),
                num(c.top),
                num(c.bottom),
                num(c.minimum),
                num(c.required),
                c.passes
            ),
            &c.diagnostics,
        )
    });
    push_metric(lines, "grid_conformance", &m.grid_conformance, |g| {
        (
            format!(
                "guides {} nodes {} key points {} max distance {} normalized {}",
                g.guide_count,
                g.node_count,
                g.evaluated_key_point_count,
                num(g.maximum_guide_distance),
                num(g.normalized_distance)
            ),
            &g.diagnostics,
        )
    });
}

fn push_metric<T>(
    lines: &mut Vec<String>,
    name: &str,
    metric: &Metric<T>,
    describe: impl Fn(&T) -> (String, &Vec<DiagnosticOutput>),
) {
    match metric {
        Metric::Measured(value) => {
            let (text, diagnostics) = describe(value);
            lines.push(format!("  {name}: {text}"));
            for d in diagnostics {
                lines.push(format!("    {}[{}]: {}", d.severity, d.code, d.message));
            }
        }
        Metric::NotApplicable { reason } => {
            lines.push(format!("  {name}: not applicable: {reason}"));
        }
    }
}

/// A rounded number, or `n/a` when it is not finite.
fn num(value: Fixed) -> String {
    match value.rounded() {
        Some(v) => format!("{v}"),
        None => "n/a".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_pipeline::{MemFs, NoConfig};

    const DOC: &str = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="ink" type="color" value="#000000"
  }
  document id="d" {
    page id="one" w=(px)20 h=(px)10 {
      rect id="r" x=(px)0 y=(px)0 w=(px)5 h=(px)5 fill=(token)"ink"
    }
  }
}
"##;

    fn run(json: bool) -> PerceiveOutcome {
        let fs = MemFs::new();
        page(
            Host::new(&fs, &NoConfig),
            DOC,
            None,
            PageSelection::One(1),
            json,
        )
        .expect("perceive")
    }

    #[test]
    fn json_is_the_page_schema_envelope() {
        let out = run(true);
        assert_eq!(out.exit_code, 0);
        let v: serde_json::Value = serde_json::from_str(&out.stdout).expect("json");
        assert_eq!(v["schema"], "zenith-perceive-page-v1");
        assert_eq!(v["decimals"], 4);
        assert_eq!(v["pages"][0]["ink"]["pixel_count"], 25);
        assert_eq!(
            v["pages"][0]["metrics"]["clearspace"]["status"],
            "not_applicable"
        );
        assert!(
            out.stdout
                .starts_with("{\n  \"schema\": \"zenith-perceive-page-v1\",\n  \"note\"")
        );
    }

    #[test]
    fn human_lists_every_metric() {
        let out = run(false);
        for name in [
            "histogram:",
            "value_zones:",
            "density_map:",
            "edge_map:",
            "optical_balance: dx",
            "clearspace: not applicable",
            "grid_conformance: not applicable",
        ] {
            assert!(out.stdout.contains(name), "missing {name}:\n{}", out.stdout);
        }
        assert!(out.stdout.contains("not a judgement of design quality"));
    }

    #[test]
    fn errors_keep_the_pipeline_exit_code() {
        let fs = MemFs::new();
        let err = page_report(
            Host::new(&fs, &NoConfig),
            "not kdl {",
            None,
            PageSelection::One(1),
        )
        .expect_err("parse error");
        assert_eq!(err.exit_code, 2);
    }

    #[test]
    fn num_prints_rounded_values_and_na() {
        assert_eq!(num(Fixed(0.123_456)), "0.1235");
        assert_eq!(num(Fixed(f64::NAN)), "n/a");
    }
}
