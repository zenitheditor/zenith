//! SVG output staging for single pages and ordered page sets.
use super::output::gate;
use super::run::{RenderRun, Stop};
use crate::commands::render::{to_svg_all_pages_with_dir_options, to_svg_with_dir_options};
impl RenderRun<'_> {
    pub(super) fn render_svg_outputs(&mut self) -> Result<(), Stop> {
        if let Some(out) = &self.args.svg {
            let art = to_svg_with_dir_options(
                self.src,
                self.args.path.parent(),
                self.args.page.unwrap_or(1),
                self.entry_options(),
            )?;
            self.record_svg_regions(out, art.page, art.rasterized_regions);
            self.write(out, art.svg, art.diagnostics, &art.import_files, "SVG")?;
        }
        if let Some(dir) = &self.args.all_pages_svg {
            let art = to_svg_all_pages_with_dir_options(
                self.src,
                self.args.path.parent(),
                self.entry_options(),
            )?;
            gate(&art.diagnostics, &art.import_files)?;
            self.directories.push(dir.to_path_buf());
            if !self.args.json {
                self.messages.push(format!(
                    "{} SVG page(s) written to '{}'",
                    art.pages.len(),
                    dir.display()
                ));
            }
            for page in art.pages {
                let path = dir.join(format!("page-{}.svg", page.page));
                self.record_svg_regions(&path, page.page, page.rasterized_regions);
                self.pending.push((path, page.svg));
            }
            self.diagnostics.extend(art.diagnostics);
            self.import_files.extend(&art.import_files);
        }
        Ok(())
    }
    fn record_svg_regions(
        &mut self,
        path: &std::path::Path,
        page: usize,
        regions: Vec<zenith_render::SvgRasterizedRegion>,
    ) {
        self.rasterized_regions
            .extend(regions.into_iter().map(|region| {
                crate::json_types::RenderRasterizedRegionJson {
                    path: path.display().to_string(),
                    format: "svg",
                    page,
                    command_start: region.command_start,
                    command_end: region.command_end,
                    reason: format!("{:?}", region.reason),
                    raster_scale: (self.raster_scale != 1.0).then_some(self.raster_scale),
                }
            }));
    }
}
