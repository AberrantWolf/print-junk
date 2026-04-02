//! Signature binding imposition (folded sheets)
//!
//! This module implements signature-based imposition using the spread model:
//! - Folio: 1 spread per side (4 pages per signature)
//! - Quarto: 2 spreads per side (8 pages per signature)
//! - Octavo: 4 spreads per side (16 pages per signature)

use super::sheet::render_sheet_spreads;
use super::{calculate_leaf_bounds, sheet_dimensions_pt};
use crate::layout::{
    ArrangementConfig, SheetSide, SpreadSheetLayout, apply_page_assignments,
    assign_pages_to_spreads, calculate_cut_edges, calculate_signature_count,
    calculate_spread_positions,
};
use crate::options::ImpositionOptions;
use crate::render::get_page_dimensions;
use crate::types::*;
use lopdf::{Dictionary, Document, Object, ObjectId};

/// Impose using signature binding (folded sheets)
pub(crate) fn impose_signature_binding(
    source: &Document,
    page_ids: &[ObjectId],
    options: &ImpositionOptions,
) -> Result<Document> {
    let total_pages = page_ids.len();
    let arrangement = options.page_arrangement;
    let config = ArrangementConfig::for_arrangement(arrangement);

    // Get source page dimensions
    let source_dimensions: Vec<(f32, f32)> = page_ids
        .iter()
        .map(|&id| {
            get_page_dimensions(source, id).unwrap_or(crate::constants::DEFAULT_PAGE_DIMENSIONS)
        })
        .collect();

    // Calculate output dimensions and leaf bounds
    let (output_width_pt, output_height_pt) = sheet_dimensions_pt(options);
    let leaf_bounds = calculate_leaf_bounds(options, output_width_pt, output_height_pt);

    // Get cut edges for the arrangement (same for all signatures)
    let cut_edges = calculate_cut_edges(arrangement);

    // Build output document
    let mut output = Document::with_version("1.7");
    let pages_tree_id = output.new_object_id();
    let mut page_refs = Vec::new();

    // Calculate number of signatures needed
    let num_signatures = calculate_signature_count(total_pages, arrangement);

    // Process each signature
    for sig_num in 0..num_signatures {
        let sig_start = sig_num * config.pages_per_signature;

        // Assign pages to spreads for this signature
        let page_assignment = assign_pages_to_spreads(arrangement, sig_start, total_pages);

        // Calculate spread positions (geometry only, same for all signatures)
        let spread_positions =
            calculate_spread_positions(arrangement, leaf_bounds, &options.margins.leaf);

        // Front side: apply page assignments to spread positions
        let front_spreads = apply_page_assignments(
            &spread_positions,
            page_assignment.for_side(SheetSide::Front),
        );
        let front_layout = SpreadSheetLayout::new(SheetSide::Front, front_spreads, leaf_bounds);

        let front_page_id = render_sheet_spreads(
            &mut output,
            source,
            page_ids,
            &front_layout,
            &cut_edges,
            &source_dimensions,
            output_width_pt,
            output_height_pt,
            pages_tree_id,
            options,
        )?;
        page_refs.push(Object::Reference(front_page_id));

        // Back side: same geometry, different page assignments
        let back_spreads =
            apply_page_assignments(&spread_positions, page_assignment.for_side(SheetSide::Back));
        let back_layout = SpreadSheetLayout::new(SheetSide::Back, back_spreads, leaf_bounds);

        let back_page_id = render_sheet_spreads(
            &mut output,
            source,
            page_ids,
            &back_layout,
            &cut_edges,
            &source_dimensions,
            output_width_pt,
            output_height_pt,
            pages_tree_id,
            options,
        )?;
        page_refs.push(Object::Reference(back_page_id));
    }

    // Finalize document
    finalize_document(&mut output, pages_tree_id, page_refs);
    Ok(output)
}


/// Create pages tree and catalog, finalize document structure
pub(crate) fn finalize_document(
    output: &mut Document,
    pages_tree_id: ObjectId,
    page_refs: Vec<Object>,
) {
    let count = page_refs.len() as i64;
    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(page_refs)),
        ("Count", Object::Integer(count)),
    ]);
    output
        .objects
        .insert(pages_tree_id, Object::Dictionary(pages_dict));

    let catalog_id = output.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_tree_id)),
    ]));

    output.trailer.set("Root", catalog_id);
}
