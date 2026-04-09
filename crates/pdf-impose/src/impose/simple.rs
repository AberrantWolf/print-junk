//! Simple 2-up binding imposition (perfect binding, side stitch, spiral)
//!
//! For non-signature bindings, pages are simply placed 2-up (one spread per sheet).
//! This uses the folio spread layout.

use super::cascade::{CascadeCell, render_cascade_page};
use super::page_source::{PageSource, XObjectCache};
use super::sheet::{create_sheet_xobject, generate_sheet_content, render_sheet_spreads};
use super::signature::finalize_document;
use crate::layout::{
    SheetSide, Spread, SpreadCutEdges, SpreadSheetLayout, calculate_spread_positions,
};
use crate::options::ImpositionOptions;
use crate::types::{PageArrangement, Result};
use lopdf::{Document, Object};

/// Impose using simple 2-up binding (perfect binding, side stitch, spiral)
///
/// Each output page has 2 source pages side by side (one spread).
pub(crate) fn impose_simple_binding(
    page_source: &PageSource,
    options: &ImpositionOptions,
) -> Result<Document> {
    let total_pages = page_source.len();

    // Calculate cell dimensions and leaf area
    let (cell_width_pt, cell_height_pt) = options.sheet_dimensions_pt();
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

    // Shared XObject cache across all sheets
    let mut xobject_cache = XObjectCache::new();

    // Pad to even number of pages
    let padded_count = total_pages.div_ceil(2) * 2;

    let cascade = options.cascade.as_ref().filter(|c| !c.is_trivial());

    if let Some(cascade) = cascade {
        // Cascade path: collect XObjects, batch into cascade pages.
        // Simple binding has no front/back — we use the same XObject for both sides
        // of CascadeCell so cascade assembly works uniformly.
        let (cascade_width_pt, cascade_height_pt) = options.cascade_sheet_dimensions_pt();
        let mut cells: Vec<CascadeCell> = Vec::new();

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

            let spread = Spread::new(left_page, right_page);
            let mut spread_pos = spread_positions[0].clone();
            spread_pos.spread = spread;

            let layout = SpreadSheetLayout::new(SheetSide::Front, vec![spread_pos], leaf_bounds);

            let content = generate_sheet_content(
                &mut output,
                page_source,
                &layout,
                &cut_edges,
                options,
                0,
                1,
                0,
                &mut xobject_cache,
            )?;
            let xobject = create_sheet_xobject(&mut output, content, cell_width_pt, cell_height_pt);

            cells.push(CascadeCell {
                front_xobject: xobject,
                back_xobject: xobject, // simple binding is single-sided
            });

            if cells.len() == cascade.cells() {
                let (front_id, _back_id) = render_cascade_page(
                    &mut output,
                    &cells,
                    cascade,
                    cell_width_pt,
                    cell_height_pt,
                    cascade_width_pt,
                    cascade_height_pt,
                    &options.margins.sheet,
                    pages_tree_id,
                );
                // Simple binding: only emit the front page (single-sided)
                page_refs.push(Object::Reference(front_id));
                cells.clear();
            }
        }

        // Flush remaining partial batch
        if !cells.is_empty() {
            let (front_id, _back_id) = render_cascade_page(
                &mut output,
                &cells,
                cascade,
                cell_width_pt,
                cell_height_pt,
                cascade_width_pt,
                cascade_height_pt,
                &options.margins.sheet,
                pages_tree_id,
            );
            page_refs.push(Object::Reference(front_id));
        }
    } else {
        // Normal path: each spread becomes its own output page
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

            let spread = Spread::new(left_page, right_page);
            let mut spread_pos = spread_positions[0].clone();
            spread_pos.spread = spread;

            let layout = SpreadSheetLayout::new(SheetSide::Front, vec![spread_pos], leaf_bounds);

            let page_id = render_sheet_spreads(
                &mut output,
                page_source,
                &layout,
                &cut_edges,
                cell_width_pt,
                cell_height_pt,
                pages_tree_id,
                options,
                0,
                1,
                0,
                &mut xobject_cache,
            )?;
            page_refs.push(Object::Reference(page_id));
        }
    }

    // Finalize document
    finalize_document(&mut output, pages_tree_id, page_refs);
    Ok(output)
}
