//! Simple 2-up binding imposition (perfect binding, side stitch, spiral)
//!
//! For non-signature bindings, pages are simply placed 2-up (one spread per sheet).
//! This uses the folio spread layout.

use super::sheet::render_sheet_spreads;
use super::signature::finalize_document;
use crate::layout::{
    SheetSide, Spread, SpreadCutEdges, SpreadSheetLayout, calculate_spread_positions,
};
use crate::options::ImpositionOptions;
use crate::render::get_page_dimensions;
use crate::types::*;
use lopdf::{Document, Object, ObjectId};

/// Impose using simple 2-up binding (perfect binding, side stitch, spiral)
///
/// Each output page has 2 source pages side by side (one spread).
pub(crate) fn impose_simple_binding(
    source: &Document,
    page_ids: &[ObjectId],
    options: &ImpositionOptions,
) -> Result<Document> {
    let total_pages = page_ids.len();

    // Get source page dimensions
    let source_dimensions: Vec<(f32, f32)> = page_ids
        .iter()
        .map(|&id| {
            get_page_dimensions(source, id).unwrap_or(crate::constants::DEFAULT_PAGE_DIMENSIONS)
        })
        .collect();

    // Calculate output dimensions and leaf area
    let (output_width_pt, output_height_pt) = options.sheet_dimensions_pt();
    let leaf_bounds = options.leaf_bounds_pt();

    // Simple 2-up uses folio layout (1 spread per sheet side)
    let spread_positions =
        calculate_spread_positions(PageArrangement::Folio, leaf_bounds, &options.margins.leaf);

    // Folio has no cuts
    let cut_edges = vec![SpreadCutEdges::none()];

    // Build output document
    let mut output = Document::with_version("1.7");
    let pages_tree_id = output.new_object_id();
    let mut page_refs = Vec::new();

    // Pad to even number of pages
    let padded_count = (total_pages + 1) / 2 * 2;

    // Process pages in pairs (one spread per output page)
    for chunk_start in (0..padded_count).step_by(2) {
        let left_page = if chunk_start < total_pages {
            Some(chunk_start)
        } else {
            None
        };
        let right_page = if chunk_start + 1 < total_pages {
            Some(chunk_start + 1)
        } else {
            None
        };

        // Create spread with page assignments
        let spread = Spread::new(left_page, right_page);

        // Clone the spread position and assign the spread
        let mut spread_pos = spread_positions[0].clone();
        spread_pos.spread = spread;

        let layout = SpreadSheetLayout::new(SheetSide::Front, vec![spread_pos], leaf_bounds);

        let page_id = render_sheet_spreads(
            &mut output,
            source,
            page_ids,
            &layout,
            &cut_edges,
            &source_dimensions,
            output_width_pt,
            output_height_pt,
            pages_tree_id,
            options,
            0,
            1,
        )?;
        page_refs.push(Object::Reference(page_id));
    }

    // Finalize document
    finalize_document(&mut output, pages_tree_id, page_refs);
    Ok(output)
}

