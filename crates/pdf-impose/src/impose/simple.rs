//! Simple 2-up binding imposition (perfect binding, side stitch, spiral)
//!
//! For non-signature bindings, pages are simply placed 2-up (one spread per sheet).
//! This uses the folio spread layout.

use super::cascade::{CascadeCell, render_cascade_page};
use super::page_source::{PageSource, XObjectCache};
use super::sheet::{create_sheet_xobject, generate_sheet_content, render_sheet_spreads};
use super::signature::finalize_document;
use crate::layout::{
    Edge, Rect, SheetSide, SheetSlot, Spread, SpreadCutEdges, SpreadSheetLayout,
    calculate_spread_positions,
};
use crate::options::ImpositionOptions;
use crate::types::{CreepConfig, PageArrangement, Result};
use lopdf::{Document, Object};

/// Build the two slots (verso, recto) for a single 2-up sheet.
///
/// Simple binding has no fold geometry: every leaf is glued at the spine, so
/// `leaf_depth` is uniformly 0 and creep doesn't apply. Spine edges still
/// matter because they drive content alignment toward the gutter.
fn slots_for_simple_spread(
    spread_bounds: Rect,
    left: Option<usize>,
    right: Option<usize>,
) -> Vec<SheetSlot> {
    let half_w = spread_bounds.width / 2.0;
    vec![
        SheetSlot {
            rect: Rect::new(
                spread_bounds.x,
                spread_bounds.y,
                half_w,
                spread_bounds.height,
            ),
            rotated: false,
            leaf_depth: 0,
            spine_edge: Edge::Right,
            source_page: left,
        },
        SheetSlot {
            rect: Rect::new(
                spread_bounds.x + half_w,
                spread_bounds.y,
                half_w,
                spread_bounds.height,
            ),
            rotated: false,
            leaf_depth: 0,
            spine_edge: Edge::Left,
            source_page: right,
        },
    ]
}

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
            let spread_bounds = spread_pos.bounds();
            spread_pos.spread = spread;

            let layout = SpreadSheetLayout::new(SheetSide::Front, vec![spread_pos], leaf_bounds);
            let slots = slots_for_simple_spread(spread_bounds, left_page, right_page);

            // Simple binding (perfect/side-stitch/spiral) glues single leaves at
            // the spine, so there is no fold geometry and creep is meaningless.
            let content = generate_sheet_content(
                &mut output,
                page_source,
                &slots,
                &layout,
                &cut_edges,
                options,
                0,
                1,
                0,
                &mut xobject_cache,
                CreepConfig::None,
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
                    options.exterior_marks_appearance,
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
                options.exterior_marks_appearance,
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
            let spread_bounds = spread_pos.bounds();
            spread_pos.spread = spread;

            let layout = SpreadSheetLayout::new(SheetSide::Front, vec![spread_pos], leaf_bounds);
            let slots = slots_for_simple_spread(spread_bounds, left_page, right_page);

            // No creep for simple binding: see comment in the cascade path above.
            let page_id = render_sheet_spreads(
                &mut output,
                page_source,
                &slots,
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
                CreepConfig::None,
            )?;
            page_refs.push(Object::Reference(page_id));
        }
    }

    // Finalize document
    finalize_document(&mut output, pages_tree_id, page_refs);
    Ok(output)
}
