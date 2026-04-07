//! Sheet rendering for imposition using spread-based layout

use crate::layout::{
    ArrangementConfig, PagePlacement, SpreadCutEdges, SpreadSheetLayout,
    calculate_spread_placements,
};
use crate::marks::{MarksConfig, MarksContext, generate_marks};
use crate::options::ImpositionOptions;
use crate::render::{create_page_xobject, render_page_numbers};
use crate::types::*;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::HashMap;

// =============================================================================
// Spread-Based Sheet Rendering
// =============================================================================

/// Render one side of a sheet using spread-based layout
pub(crate) fn render_sheet_spreads(
    output: &mut Document,
    source: &Document,
    source_page_ids: &[ObjectId],
    layout: &SpreadSheetLayout,
    cut_edges: &[SpreadCutEdges],
    source_dimensions: &[(f32, f32)],
    sheet_width_pt: f32,
    sheet_height_pt: f32,
    parent_pages_id: ObjectId,
    options: &ImpositionOptions,
    signature_index: usize,
    total_signatures: usize,
) -> Result<ObjectId> {
    let mut page_dict = create_page_dict(parent_pages_id, sheet_width_pt, sheet_height_pt);

    let mut content_ops = Vec::new();
    let mut xobjects = Dictionary::new();
    let mut fonts = Dictionary::new();
    let mut xobject_cache: HashMap<ObjectId, ObjectId> = HashMap::new();

    // Calculate page placements from spreads
    let placements = calculate_spread_placements(
        &layout.spreads,
        cut_edges,
        source_dimensions,
        &options.margins.leaf,
        options.scaling_mode,
        layout.side,
    );

    // Render each page placement
    for (idx, placement) in placements.iter().enumerate() {
        if let Some(source_idx) = placement.source_page {
            if source_idx < source_page_ids.len() {
                let source_page_id = source_page_ids[source_idx];
                let xobject_name = format!("P{}", idx);

                // Create XObject
                let xobject_id =
                    create_page_xobject(output, source, source_page_id, &mut xobject_cache)?;
                xobjects.set(xobject_name.as_bytes(), Object::Reference(xobject_id));

                // Generate placement command
                content_ops.push(generate_placement_cmd(&xobject_name, placement));
            }
        }
    }

    // Generate printer's marks
    if options.marks.any_enabled() {
        let config = ArrangementConfig::for_arrangement(options.page_arrangement);
        let marks_config = MarksConfig::from_layout(
            &layout.spreads,
            &config,
            &layout.leaf_bounds,
            crate::constants::mm_to_pt(options.margins.leaf.trim_allowance_mm),
        );
        let marks_ctx = MarksContext {
            binding_type: options.binding_type,
            signature_index,
            total_signatures,
            sewing_config: options.sewing_config,
        };
        content_ops.push(generate_marks(&options.marks, &marks_config, Some(&marks_ctx)));
    }

    // Add page numbers
    if options.add_page_numbers {
        let (font_ops, font_id) = render_page_numbers(
            output,
            &placements,
            options.page_number_start,
        );
        content_ops.push(font_ops);
        fonts.set("F1", Object::Reference(font_id));
    }

    // Build resources
    let mut resources = Dictionary::new();
    resources.set("XObject", Object::Dictionary(xobjects));
    if !fonts.is_empty() {
        resources.set("Font", Object::Dictionary(fonts));
    }

    // Create content stream
    let content = content_ops.join("");
    let content_id = output.add_object(Stream::new(Dictionary::new(), content.into_bytes()));

    page_dict.set("Contents", Object::Reference(content_id));
    page_dict.set("Resources", Object::Dictionary(resources));

    Ok(output.add_object(page_dict))
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Create a basic page dictionary
fn create_page_dict(parent_id: ObjectId, width: f32, height: f32) -> Dictionary {
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"Page".to_vec()));
    dict.set("Parent", Object::Reference(parent_id));
    dict.set(
        "MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Real(width),
            Object::Real(height),
        ]),
    );
    dict
}

/// Generate PDF command to place an XObject
fn generate_placement_cmd(xobject_name: &str, placement: &PagePlacement) -> String {
    let rect = &placement.content_rect;

    if placement.is_rotated() {
        // 180° rotation: matrix is [-scale 0 0 -scale tx ty]
        let rot_x = rect.x + rect.width;
        let rot_y = rect.y + rect.height;
        format!(
            "q {} 0 0 {} {} {} cm /{} Do Q\n",
            -placement.scale, -placement.scale, rot_x, rot_y, xobject_name
        )
    } else {
        format!(
            "q {} 0 0 {} {} {} cm /{} Do Q\n",
            placement.scale, placement.scale, rect.x, rect.y, xobject_name
        )
    }
}

