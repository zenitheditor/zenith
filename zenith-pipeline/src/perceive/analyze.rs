//! The metrics of one page raster.

use std::collections::BTreeMap;

use zenith_core::{Document, Page};
use zenith_geometry::RectBounds;
use zenith_perception::{
    ClearspaceInput, CompoundGridConformanceInput, DensityCell, OpticalBalanceInput,
    PerceptionReport, ValueZone, VectorPathContourInput, analyze, clearspace,
    compound_grid_conformance, optical_balance,
};
use zenith_raster::Surface;
use zenith_scene::CompiledBox;

use super::inputs::{
    NO_GUIDES, NO_INK, NO_NODES, clearspace_required, construction_guides, node_contours,
};
use super::output::{
    BoundsOutput, ClearspaceOutput, DensityCellOutput, DensityOutput, DensityRatiosOutput,
    EdgeOutput, Fixed, GridConformanceOutput, HistogramOutput, InkOutput, Metric,
    OpticalBalanceOutput, PageMetrics, ValueZonesOutput, ZoneOutput, diagnostics_output,
};
use super::raster::{Ink, hex};

/// What one page's metrics read.
#[derive(Clone, Copy)]
pub(crate) struct PageInputs<'a> {
    pub(crate) doc: &'a Document,
    pub(crate) page: &'a Page,
    pub(crate) boxes: &'a BTreeMap<String, CompiledBox>,
    pub(crate) surface: &'a Surface,
    pub(crate) ink: Ink,
}

/// The ink entry of the envelope.
pub(crate) fn ink_output(ink: Ink) -> InkOutput {
    InkOutput {
        background: hex(ink.background),
        pixel_count: ink.pixel_count,
        bounds: ink_bounds(ink).map(bounds_output),
    }
}

/// Every metric of one page.
pub(crate) fn page_metrics(inputs: PageInputs<'_>) -> PageMetrics {
    let PerceptionReport {
        histogram,
        value_zones,
        density_map,
        edge_map,
    } = analyze(inputs.surface);
    let container = RectBounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: f64::from(inputs.surface.width()),
        max_y: f64::from(inputs.surface.height()),
    };
    let subject = ink_bounds(inputs.ink);
    PageMetrics {
        histogram: Metric::Measured(HistogramOutput {
            bins: histogram.bins.to_vec(),
            total_pixels: histogram.total_pixels,
            min: histogram.min.into(),
            max: histogram.max.into(),
            mean: histogram.mean.into(),
            diagnostics: Vec::new(),
        }),
        value_zones: Metric::Measured(ValueZonesOutput {
            zones: value_zones
                .zones
                .iter()
                .map(|z| ZoneOutput {
                    zone: zone_name(z.zone),
                    count: z.count,
                    min: z.min.into(),
                    max: z.max.into(),
                    mean: z.mean.into(),
                    spread: z.spread.into(),
                })
                .collect(),
            diagnostics: diagnostics_output(&value_zones.diagnostics),
        }),
        density_map: Metric::Measured(DensityOutput {
            columns: density_map.columns,
            rows: density_map.rows,
            total_energy: density_map.total_energy.into(),
            focal_x: density_map.focal_x.into(),
            focal_y: density_map.focal_y.into(),
            strongest_cell: density_map.strongest_cell.as_ref().map(cell_output),
            ratios: DensityRatiosOutput {
                top_70: density_map.ratios.top_70.into(),
                next_20: density_map.ratios.next_20.into(),
                remaining_10: density_map.ratios.remaining_10.into(),
            },
            cells: density_map.cells.iter().map(cell_output).collect(),
            diagnostics: diagnostics_output(&density_map.diagnostics),
        }),
        edge_map: Metric::Measured(EdgeOutput {
            count: edge_map.count,
            max: edge_map.max.into(),
            mean: edge_map.mean.into(),
            hard_count: edge_map.hard_count,
            lost_count: edge_map.lost_count,
            diagnostics: diagnostics_output(&edge_map.diagnostics),
        }),
        optical_balance: optical_balance_metric(subject, container),
        clearspace: clearspace_metric(inputs, subject, container),
        grid_conformance: grid_conformance_metric(inputs),
    }
}

/// Ink centre against the page centre. Not applicable on a flat page.
fn optical_balance_metric(
    subject: Option<RectBounds>,
    container: RectBounds,
) -> Metric<OpticalBalanceOutput> {
    let Some(subject) = subject else {
        return Metric::NotApplicable { reason: NO_INK };
    };
    let report = optical_balance(OpticalBalanceInput { subject, container });
    Metric::Measured(OpticalBalanceOutput {
        subject: bounds_output(subject),
        container: bounds_output(container),
        center_dx: report.center_dx.into(),
        center_dy: report.center_dy.into(),
        normalized_dx: report.normalized_dx.into(),
        normalized_dy: report.normalized_dy.into(),
        offset_magnitude: report.offset_magnitude.into(),
        diagnostics: diagnostics_output(&report.diagnostics),
    })
}

/// Ink distance to the page edges against the smallest declared margin.
/// Not applicable without four declared margins, or on a flat page.
fn clearspace_metric(
    inputs: PageInputs<'_>,
    subject: Option<RectBounds>,
    container: RectBounds,
) -> Metric<ClearspaceOutput> {
    let required = match clearspace_required(inputs.doc, inputs.page) {
        Ok(required) => required,
        Err(reason) => return Metric::NotApplicable { reason },
    };
    let Some(subject) = subject else {
        return Metric::NotApplicable { reason: NO_INK };
    };
    let report = clearspace(ClearspaceInput {
        subject,
        container,
        required,
    });
    Metric::Measured(ClearspaceOutput {
        subject: bounds_output(subject),
        container: bounds_output(container),
        left: report.left.into(),
        right: report.right.into(),
        top: report.top.into(),
        bottom: report.bottom.into(),
        minimum: report.minimum.into(),
        required: report.required.into(),
        passes: report.passes,
        diagnostics: diagnostics_output(&report.diagnostics),
    })
}

/// Top-level node box key points against the declared construction guides.
/// Not applicable without a guide or a node box.
fn grid_conformance_metric(inputs: PageInputs<'_>) -> Metric<GridConformanceOutput> {
    let guides = construction_guides(inputs.page);
    if guides.is_empty() {
        return Metric::NotApplicable { reason: NO_GUIDES };
    }
    let contours = node_contours(inputs.page, inputs.boxes);
    if contours.is_empty() {
        return Metric::NotApplicable { reason: NO_NODES };
    }
    let contour_inputs: Vec<VectorPathContourInput<'_>> = contours
        .iter()
        .map(|anchors| VectorPathContourInput {
            anchors,
            closed: true,
        })
        .collect();
    let report = compound_grid_conformance(CompoundGridConformanceInput {
        contours: &contour_inputs,
        guides: &guides,
    });
    Metric::Measured(GridConformanceOutput {
        guide_count: report.guide_count,
        node_count: contours.len(),
        evaluated_key_point_count: report.evaluated_key_point_count,
        maximum_guide_distance: report.maximum_guide_distance.into(),
        normalized_distance: report.normalized_distance.into(),
        diagnostics: diagnostics_output(&report.diagnostics),
    })
}

fn ink_bounds(ink: Ink) -> Option<RectBounds> {
    ink.bounds.map(|(min_x, min_y, max_x, max_y)| RectBounds {
        min_x: f64::from(min_x),
        min_y: f64::from(min_y),
        max_x: f64::from(max_x),
        max_y: f64::from(max_y),
    })
}

fn bounds_output(bounds: RectBounds) -> BoundsOutput {
    BoundsOutput {
        min_x: Fixed(bounds.min_x),
        min_y: Fixed(bounds.min_y),
        max_x: Fixed(bounds.max_x),
        max_y: Fixed(bounds.max_y),
    }
}

fn cell_output(cell: &DensityCell) -> DensityCellOutput {
    DensityCellOutput {
        row: cell.row,
        column: cell.column,
        x_start: cell.x_start,
        y_start: cell.y_start,
        x_end: cell.x_end,
        y_end: cell.y_end,
        pixel_count: cell.pixel_count,
        energy: cell.energy.into(),
    }
}

fn zone_name(zone: ValueZone) -> &'static str {
    match zone {
        ValueZone::Shadow => "shadow",
        ValueZone::Midtone => "midtone",
        ValueZone::Highlight => "highlight",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};
    use zenith_raster::LinearRgba;

    fn doc(page_attrs: &str, body: &str) -> Document {
        let src = format!(
            "zenith version=1 {{\n  document id=\"d\" {{\n    page id=\"p\" w=(px)4 h=(px)4 {page_attrs} {{\n{body}\n    }}\n  }}\n}}\n"
        );
        KdlAdapter.parse(src.as_bytes()).expect("parse")
    }

    fn surface() -> Surface {
        let white = LinearRgba::straight(1.0, 1.0, 1.0, 1.0).unwrap();
        let black = LinearRgba::straight(0.0, 0.0, 0.0, 1.0).unwrap();
        let mut s = Surface::filled(4, 4, white).unwrap();
        s.set(0, 0, black).unwrap();
        s
    }

    fn ink() -> Ink {
        Ink {
            background: [255, 255, 255, 255],
            pixel_count: 1,
            bounds: Some((0, 0, 1, 1)),
        }
    }

    #[test]
    fn plain_page_measures_raster_metrics_and_skips_declared_ones() {
        let d = doc("", "");
        let s = surface();
        let boxes = BTreeMap::new();
        let m = page_metrics(PageInputs {
            doc: &d,
            page: &d.body.pages[0],
            boxes: &boxes,
            surface: &s,
            ink: ink(),
        });
        let Metric::Measured(h) = &m.histogram else {
            panic!("histogram must be measured");
        };
        assert_eq!(h.total_pixels, 16);
        let Metric::Measured(o) = &m.optical_balance else {
            panic!("optical balance must be measured");
        };
        assert_eq!(o.center_dx, Fixed(-1.5));
        assert_eq!(
            m.clearspace,
            Metric::NotApplicable {
                reason: super::super::inputs::NO_MARGINS
            }
        );
        assert_eq!(
            m.grid_conformance,
            Metric::NotApplicable { reason: NO_GUIDES }
        );
    }

    #[test]
    fn declared_margins_run_clearspace() {
        let d = doc(
            "margin-top=(px)1 margin-bottom=(px)1 margin-inner=(px)1 margin-outer=(px)1",
            "",
        );
        let s = surface();
        let boxes = BTreeMap::new();
        let m = page_metrics(PageInputs {
            doc: &d,
            page: &d.body.pages[0],
            boxes: &boxes,
            surface: &s,
            ink: ink(),
        });
        let Metric::Measured(c) = &m.clearspace else {
            panic!("clearspace must be measured");
        };
        assert_eq!(c.minimum, Fixed(0.0));
        assert_eq!(c.required, Fixed(1.0));
        assert!(!c.passes);
        assert_eq!(c.diagnostics[0].code, "clearspace.insufficient");
    }

    #[test]
    fn flat_page_skips_ink_metrics() {
        let d = doc("", "");
        let s = surface();
        let boxes = BTreeMap::new();
        let flat = Ink {
            background: [255, 255, 255, 255],
            pixel_count: 0,
            bounds: None,
        };
        let m = page_metrics(PageInputs {
            doc: &d,
            page: &d.body.pages[0],
            boxes: &boxes,
            surface: &s,
            ink: flat,
        });
        assert_eq!(m.optical_balance, Metric::NotApplicable { reason: NO_INK });
        assert_eq!(ink_output(flat).bounds, None);
    }

    #[test]
    fn guides_without_node_boxes_skip_grid_conformance() {
        let d = doc(
            "",
            r#"      construction {
        guide id="g" type="segment" x1=(px)0 y1=(px)0 x2=(px)4 y2=(px)0
      }"#,
        );
        let s = surface();
        let boxes = BTreeMap::new();
        let m = page_metrics(PageInputs {
            doc: &d,
            page: &d.body.pages[0],
            boxes: &boxes,
            surface: &s,
            ink: ink(),
        });
        assert_eq!(
            m.grid_conformance,
            Metric::NotApplicable { reason: NO_NODES }
        );
    }

    #[test]
    fn zone_names_are_lowercase() {
        assert_eq!(zone_name(ValueZone::Shadow), "shadow");
        assert_eq!(zone_name(ValueZone::Midtone), "midtone");
        assert_eq!(zone_name(ValueZone::Highlight), "highlight");
    }
}
