use zenith_raster::Surface;

use crate::diagnostic::{PerceptionDiagnostic, PerceptionSeverity};
use crate::scalar::{luminance, mean_luminance};

#[derive(Debug, Clone, PartialEq)]
pub struct DensityCell {
    pub row: u32,
    pub column: u32,
    pub x_start: u32,
    pub y_start: u32,
    pub x_end: u32,
    pub y_end: u32,
    pub pixel_count: u64,
    pub energy: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DensityRatioSummary {
    pub top_70: f32,
    pub next_20: f32,
    pub remaining_10: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DensityReport {
    pub columns: u32,
    pub rows: u32,
    pub cells: Vec<DensityCell>,
    pub total_energy: f32,
    pub strongest_cell: Option<DensityCell>,
    pub focal_x: f32,
    pub focal_y: f32,
    pub ratios: DensityRatioSummary,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

pub fn density_map(surface: &Surface) -> DensityReport {
    let width = surface.width();
    let height = surface.height();
    let columns = width.min(4);
    let rows = height.min(4);
    let mean = mean_luminance(surface);
    let mut cells = build_cells(width, height, columns, rows);

    for y in 0..height {
        for x in 0..width {
            let Some(pixel) = surface.get(x, y) else {
                continue;
            };
            let column = cell_axis_index(x, width, columns);
            let row = cell_axis_index(y, height, rows);
            let energy = pixel.a() * (luminance(pixel) - mean).abs();
            if let Some(index) = cell_index(row, column, columns) {
                if let Some(cell) = cells.get_mut(index) {
                    cell.energy += energy;
                    cell.pixel_count += 1;
                }
            }
        }
    }

    let total_energy = cells.iter().map(|cell| cell.energy).sum::<f32>();
    let strongest_cell = strongest_cell(&cells);
    let (focal_x, focal_y) = focal_location(&strongest_cell);
    let diagnostics = density_diagnostics(total_energy, &strongest_cell, columns, rows);

    DensityReport {
        columns,
        rows,
        ratios: ratio_summary(&cells, total_energy),
        cells,
        total_energy,
        strongest_cell,
        focal_x,
        focal_y,
        diagnostics,
    }
}

fn build_cells(width: u32, height: u32, columns: u32, rows: u32) -> Vec<DensityCell> {
    let mut cells = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            cells.push(DensityCell {
                row,
                column,
                x_start: axis_start(column, width, columns),
                y_start: axis_start(row, height, rows),
                x_end: axis_start(column + 1, width, columns),
                y_end: axis_start(row + 1, height, rows),
                pixel_count: 0,
                energy: 0.0,
            });
        }
    }
    cells
}

fn strongest_cell(cells: &[DensityCell]) -> Option<DensityCell> {
    let mut strongest: Option<DensityCell> = None;
    for cell in cells {
        let replace = match &strongest {
            Some(current) => cell.energy > current.energy,
            None => true,
        };
        if replace {
            strongest = Some(cell.clone());
        }
    }
    strongest
}

fn focal_location(strongest_cell: &Option<DensityCell>) -> (f32, f32) {
    match strongest_cell {
        Some(cell) => (
            (cell.x_start as f32 + cell.x_end as f32) * 0.5,
            (cell.y_start as f32 + cell.y_end as f32) * 0.5,
        ),
        None => (0.0, 0.0),
    }
}

fn density_diagnostics(
    total_energy: f32,
    strongest_cell: &Option<DensityCell>,
    columns: u32,
    rows: u32,
) -> Vec<PerceptionDiagnostic> {
    let Some(cell) = strongest_cell else {
        return Vec::new();
    };
    if total_energy > 0.0 && is_border_cell(cell, columns, rows) {
        return vec![PerceptionDiagnostic::new(
            "density.focal_drift",
            PerceptionSeverity::Info,
            "strongest density cell is on the outer grid border",
        )];
    }
    Vec::new()
}

fn is_border_cell(cell: &DensityCell, columns: u32, rows: u32) -> bool {
    cell.row == 0 || cell.column == 0 || cell.row + 1 == rows || cell.column + 1 == columns
}

fn ratio_summary(cells: &[DensityCell], total_energy: f32) -> DensityRatioSummary {
    if total_energy <= 0.0 {
        return DensityRatioSummary {
            top_70: 0.0,
            next_20: 0.0,
            remaining_10: 0.0,
        };
    }

    let mut sorted = cells.to_vec();
    sorted.sort_by(|left, right| {
        right
            .energy
            .total_cmp(&left.energy)
            .then_with(|| left.row.cmp(&right.row))
            .then_with(|| left.column.cmp(&right.column))
    });

    let len = sorted.len();
    let top_end = percent_ceil(len, 70).min(len);
    let next_end = top_end.saturating_add(percent_ceil(len, 20)).min(len);

    let mut top = 0.0;
    let mut next = 0.0;
    let mut remaining = 0.0;
    for (index, cell) in sorted.iter().enumerate() {
        if index < top_end {
            top += cell.energy;
        } else if index < next_end {
            next += cell.energy;
        } else {
            remaining += cell.energy;
        }
    }

    DensityRatioSummary {
        top_70: top / total_energy,
        next_20: next / total_energy,
        remaining_10: remaining / total_energy,
    }
}

fn percent_ceil(len: usize, percent: usize) -> usize {
    len.saturating_mul(percent).saturating_add(99) / 100
}

fn cell_index(row: u32, column: u32, columns: u32) -> Option<usize> {
    let index = u64::from(row) * u64::from(columns) + u64::from(column);
    usize::try_from(index).ok()
}

fn cell_axis_index(position: u32, size: u32, cells: u32) -> u32 {
    let scaled = (u64::from(position) * u64::from(cells)) / u64::from(size);
    match u32::try_from(scaled) {
        Ok(value) => value.min(cells.saturating_sub(1)),
        Err(_) => cells.saturating_sub(1),
    }
}

fn axis_start(index: u32, size: u32, cells: u32) -> u32 {
    let scaled = (u64::from(index) * u64::from(size)) / u64::from(cells);
    match u32::try_from(scaled) {
        Ok(value) => value,
        Err(_) => size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_raster::{LinearRgba, Surface};

    #[test]
    fn density_reports_deterministic_ratio_and_border_focal_cell() {
        let pixels = vec![
            gray(1.0),
            LinearRgba::TRANSPARENT,
            LinearRgba::TRANSPARENT,
            LinearRgba::TRANSPARENT,
            LinearRgba::TRANSPARENT,
            LinearRgba::TRANSPARENT,
            LinearRgba::TRANSPARENT,
            LinearRgba::TRANSPARENT,
            LinearRgba::TRANSPARENT,
        ];
        let surface = Surface::from_pixels(3, 3, pixels).unwrap();

        let report = density_map(&surface);

        assert_eq!(report.columns, 3);
        assert_eq!(report.rows, 3);
        assert_eq!(
            report
                .strongest_cell
                .as_ref()
                .map(|cell| (cell.row, cell.column)),
            Some((0, 0))
        );
        assert_eq!(report.focal_x, 0.5);
        assert_eq!(report.focal_y, 0.5);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "density.focal_drift");
        assert!((report.ratios.top_70 - 1.0).abs() < 0.000_001);
        assert_eq!(report.ratios.next_20, 0.0);
        assert_eq!(report.ratios.remaining_10, 0.0);
    }

    fn gray(value: f32) -> LinearRgba {
        LinearRgba::straight(value, value, value, 1.0).unwrap()
    }
}
