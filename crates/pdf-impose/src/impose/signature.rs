//! Signature binding imposition (folded sheets)
//!
//! This module implements signature-based imposition using the spread model:
//! - Folio: 1 spread per side (4 pages per signature)
//! - Quarto: 2 spreads per side (8 pages per signature)
//! - Octavo: 4 spreads per side (16 pages per signature)

use super::cascade::{CascadeCell, render_cascade_page};
use super::page_source::{PageSource, XObjectCache};
use super::sheet::{create_sheet_xobject, generate_sheet_content, render_sheet_spreads};
use crate::layout::{
    SheetSide, SpreadSheetLayout, apply_page_assignments, assign_pages_to_spreads,
    calculate_cut_edges, calculate_signature_count, calculate_spread_positions,
};
use crate::options::ImpositionOptions;
use crate::types::Result;
use lopdf::{Dictionary, Document, Object, ObjectId};

/// Impose using signature binding (folded sheets)
pub(crate) fn impose_signature_binding(
    page_source: &PageSource,
    options: &ImpositionOptions,
) -> Result<Document> {
    let total_pages = page_source.len();
    let arrangement = options.page_arrangement;
    let pages_per_sig = options.pages_per_signature();

    // Cell dimensions (= sheet dimensions; cascade derives these from the larger sheet)
    let (cell_width_pt, cell_height_pt) = options.sheet_dimensions_pt();
    let leaf_bounds = options.leaf_bounds_pt();

    // Get cut edges for the arrangement (same for all sheets)
    let cut_edges = calculate_cut_edges(arrangement);

    // Spread positions are the same for every sheet (determined by fold type)
    let spread_positions =
        calculate_spread_positions(arrangement, leaf_bounds, &options.margins.leaf);

    // Build output document
    let mut output = Document::with_version("1.7");
    let pages_tree_id = output.new_object_id();
    let mut page_refs = Vec::new();

    // Shared XObject cache across all sheets
    let mut xobject_cache = XObjectCache::new();

    // Calculate number of signatures needed
    let num_signatures = calculate_signature_count(total_pages, pages_per_sig);

    let cascade = options.cascade.as_ref().filter(|c| !c.is_trivial());

    if let Some(cascade) = cascade {
        // Cascade path: collect cells, then batch into cascade pages
        let (cascade_width_pt, cascade_height_pt) = options.cascade_sheet_dimensions_pt();
        let mut cells: Vec<CascadeCell> = Vec::new();

        for sig_num in 0..num_signatures {
            let sig_start = sig_num * pages_per_sig;
            let sheet_assignments = assign_pages_to_spreads(
                arrangement,
                options.sheets_per_signature,
                sig_start,
                total_pages,
            );

            for (sheet_idx, sheet_assignment) in sheet_assignments.iter().enumerate() {
                // Front
                let front_spreads = apply_page_assignments(
                    &spread_positions,
                    sheet_assignment.for_side(SheetSide::Front),
                );
                let front_layout =
                    SpreadSheetLayout::new(SheetSide::Front, front_spreads, leaf_bounds);
                let front_content = generate_sheet_content(
                    &mut output,
                    page_source,
                    &front_layout,
                    &cut_edges,
                    options,
                    sig_num,
                    num_signatures,
                    sheet_idx,
                    &mut xobject_cache,
                )?;
                let front_xobject =
                    create_sheet_xobject(&mut output, front_content, cell_width_pt, cell_height_pt);

                // Back
                let back_spreads = apply_page_assignments(
                    &spread_positions,
                    sheet_assignment.for_side(SheetSide::Back),
                );
                let back_layout =
                    SpreadSheetLayout::new(SheetSide::Back, back_spreads, leaf_bounds);
                let back_content = generate_sheet_content(
                    &mut output,
                    page_source,
                    &back_layout,
                    &cut_edges,
                    options,
                    sig_num,
                    num_signatures,
                    sheet_idx,
                    &mut xobject_cache,
                )?;
                let back_xobject =
                    create_sheet_xobject(&mut output, back_content, cell_width_pt, cell_height_pt);

                cells.push(CascadeCell {
                    front_xobject,
                    back_xobject,
                });

                // When batch is full, render cascade page
                if cells.len() == cascade.cells() {
                    let (front_id, back_id) = render_cascade_page(
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
                    page_refs.push(Object::Reference(back_id));
                    cells.clear();
                }
            }
        }

        // Flush remaining partial batch
        if !cells.is_empty() {
            let (front_id, back_id) = render_cascade_page(
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
            page_refs.push(Object::Reference(back_id));
        }
    } else {
        // Normal path: each sheet becomes its own output page
        for sig_num in 0..num_signatures {
            let sig_start = sig_num * pages_per_sig;
            let sheet_assignments = assign_pages_to_spreads(
                arrangement,
                options.sheets_per_signature,
                sig_start,
                total_pages,
            );

            for (sheet_idx, sheet_assignment) in sheet_assignments.iter().enumerate() {
                // Front side
                let front_spreads = apply_page_assignments(
                    &spread_positions,
                    sheet_assignment.for_side(SheetSide::Front),
                );
                let front_layout =
                    SpreadSheetLayout::new(SheetSide::Front, front_spreads, leaf_bounds);

                let front_page_id = render_sheet_spreads(
                    &mut output,
                    page_source,
                    &front_layout,
                    &cut_edges,
                    cell_width_pt,
                    cell_height_pt,
                    pages_tree_id,
                    options,
                    sig_num,
                    num_signatures,
                    sheet_idx,
                    &mut xobject_cache,
                )?;
                page_refs.push(Object::Reference(front_page_id));

                // Back side
                let back_spreads = apply_page_assignments(
                    &spread_positions,
                    sheet_assignment.for_side(SheetSide::Back),
                );
                let back_layout =
                    SpreadSheetLayout::new(SheetSide::Back, back_spreads, leaf_bounds);

                let back_page_id = render_sheet_spreads(
                    &mut output,
                    page_source,
                    &back_layout,
                    &cut_edges,
                    cell_width_pt,
                    cell_height_pt,
                    pages_tree_id,
                    options,
                    sig_num,
                    num_signatures,
                    sheet_idx,
                    &mut xobject_cache,
                )?;
                page_refs.push(Object::Reference(back_page_id));
            }
        }
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
