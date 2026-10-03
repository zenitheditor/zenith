//! Intrinsic size of an `instance`: the content bounds of its expanded,
//! lowered component subtree.
//!
//! The expansion is the one compile renders (overrides applied, ids and
//! imported asset refs prefixed), and its layout frames lower before the
//! bounds are read, so the hug size matches the drawn component.

use zenith_core::InstanceNode;

use crate::layout::lower_nodes;

use super::super::container::{expand_imported, expand_local, group_children_bounds};
use super::super::font_ns::NamespacedFontProvider;
use super::super::imports::{ImportSource, parse_import_source};
use super::IntrinsicEnv;

/// The deepest instance-in-component nesting that measures. A deeper (or a
/// self-referencing) chain has no intrinsic size.
const MAX_INSTANCE_NESTING: usize = 8;

impl IntrinsicEnv<'_> {
    /// The content bounds `(min_x, min_y, w, h)` of `instance`'s expanded
    /// component, in the component's local space (the instance origin is
    /// `(0, 0)`).
    ///
    /// `None` for a hidden instance, an unknown or unexpandable component, a
    /// component without positive-area content, or nesting deeper than
    /// [`MAX_INSTANCE_NESTING`].
    pub(crate) fn instance_bounds(&self, instance: &InstanceNode) -> Option<(f64, f64, f64, f64)> {
        if instance.visible == Some(false) || self.nesting >= MAX_INSTANCE_NESTING {
            return None;
        }
        let bounds = match instance.source.as_deref() {
            Some(source) => self.imported_bounds(instance, source)?,
            None => {
                let component = self.components.get(instance.component.as_deref()?)?;
                let mut children = expand_local(component, instance);
                let env = IntrinsicEnv {
                    nesting: self.nesting + 1,
                    ..*self
                };
                let _ = lower_nodes(&mut children, env, None);
                group_children_bounds(&children, 0.0, 0.0, self.resolved)?
            }
        };
        (bounds.2 > 0.0 && bounds.3 > 0.0).then_some(bounds)
    }

    /// [`IntrinsicEnv::instance_bounds`] of an imported component, measured
    /// in the import's own token, style, component, and font scope.
    fn imported_bounds(
        &self,
        instance: &InstanceNode,
        source: &str,
    ) -> Option<(f64, f64, f64, f64)> {
        if !self.imports.is_enabled() {
            return None;
        }
        let ImportSource::Component {
            import_id,
            component_id,
        } = parse_import_source(source)
        else {
            return None;
        };
        let imported = self.imports.get(import_id)?;
        let component = imported.components.get(component_id)?;
        let mut children = expand_imported(component, instance, import_id, self.resolved);
        let fonts = NamespacedFontProvider::new(self.fonts, import_id);
        let env = IntrinsicEnv {
            resolved: &imported.resolved,
            style_map: &imported.style_map,
            fonts: &fonts,
            components: &imported.components,
            page_block_styles: &[],
            doc_block_styles: &imported.document.body.block_styles,
            nesting: self.nesting + 1,
            ..*self
        };
        let _ = lower_nodes(&mut children, env, None);
        group_children_bounds(&children, 0.0, 0.0, &imported.resolved)
    }
}
