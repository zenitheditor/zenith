//! Per-document page compiler: document-wide lookups and pre-passes built once.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, FontProvider, Style};
use zenith_layout::{FontFaceStore, RustybuzzEngine};

use super::super::chain::{ChainAssignments, resolve_chains_document};
use super::super::field::{SectionAssignment, build_page_index_map, build_section_assignments};
use super::super::table_flow::{TableFlowAssignments, resolve_table_flows};
use super::super::{ComponentMap, MasterMap};
use super::fonts::FontsRef;
use super::prep::DocumentPrep;

/// Compiles pages of one prepared document.
///
/// [`PageCompiler::new`] builds the style, component, and master maps, the
/// page-ref index, the font face store, the text-chain and table-flow
/// pre-passes, and the section assignments once. [`PageCompiler::compile_page`]
/// then compiles any page from them and returns the same result as
/// [`compile_page`](crate::compile::compile_page).
///
/// The compiler is `Sync` when the provider type `F` is `Sync`. Each
/// `compile_page` call builds its own shaping engine, so pages can compile on
/// separate threads.
pub struct PageCompiler<'p, F: ?Sized + FontProvider = dyn FontProvider> {
    pub(super) prep: &'p DocumentPrep<'p>,
    pub(super) fonts: &'p F,
    pub(super) font_faces: FontFaceStore,
    pub(super) style_map: BTreeMap<&'p str, &'p Style>,
    pub(super) component_map: ComponentMap<'p>,
    pub(super) master_map: MasterMap<'p>,
    pub(super) page_index_by_node_id: BTreeMap<String, usize>,
    pub(super) chains: ChainAssignments,
    pub(super) flows: TableFlowAssignments,
    pub(super) section_assignments: Vec<Option<SectionAssignment<'p>>>,
    /// Chain and table-flow diagnostics. Only page 0 reports them.
    pub(super) page0_diagnostics: Vec<Diagnostic>,
}

impl<'p, F: ?Sized + FontProvider> PageCompiler<'p, F> {
    /// Build the document-wide page lookups and pre-passes for `prep`.
    ///
    /// Text chains and table flows shape here, once, with `fonts`.
    #[must_use]
    pub fn new(prep: &'p DocumentPrep<'p>, fonts: &'p F) -> Self {
        let doc = prep.document();
        let fonts_ref = FontsRef(fonts);
        let dyn_fonts: &dyn FontProvider = &fonts_ref;

        let style_map: BTreeMap<&str, &Style> = doc
            .styles
            .styles
            .iter()
            .map(|s| (s.id.as_str(), s))
            .collect();

        // First declaration wins on a duplicate id (the validator flags it).
        let mut component_map: ComponentMap = BTreeMap::new();
        for comp in &doc.components {
            component_map.entry(comp.id.as_str()).or_insert(comp);
        }
        let mut master_map: MasterMap = BTreeMap::new();
        for master in &doc.masters {
            master_map.entry(master.id.as_str()).or_insert(master);
        }
        let page_index_by_node_id = build_page_index_map(doc);

        // Each face parses at most once per engine. This engine serves the
        // pre-passes only. Each page compile builds its own engine over the
        // same store, so shaped output is identical.
        let font_faces = FontFaceStore::new(dyn_fonts);
        let mut page0_diagnostics: Vec<Diagnostic> = Vec::new();
        let (chains, flows) = {
            let engine = RustybuzzEngine::new(&font_faces);
            let chains = resolve_chains_document(
                doc,
                &prep.resolved,
                &style_map,
                dyn_fonts,
                &engine,
                &prep.md_blocks,
                &mut page0_diagnostics,
            );
            let flows = resolve_table_flows(
                doc,
                &prep.resolved,
                &style_map,
                dyn_fonts,
                &engine,
                &mut page0_diagnostics,
            );
            (chains, flows)
        };

        let section_assignments = build_section_assignments(doc);

        Self {
            prep,
            fonts,
            font_faces,
            style_map,
            component_map,
            master_map,
            page_index_by_node_id,
            chains,
            flows,
            section_assignments,
            page0_diagnostics,
        }
    }

    /// The prepared document this compiler reads.
    #[must_use]
    pub fn prep(&self) -> &'p DocumentPrep<'p> {
        self.prep
    }

    /// Number of pages in the compiled document.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.prep.page_count()
    }
}
