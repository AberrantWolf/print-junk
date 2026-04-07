//! Output page rendering for imposition
//!
//! This module provides a standalone function for creating imposed PDF pages.
//! It's exported as public API but the main imposition workflow uses
//! `impose/sheet.rs` internally.

use crate::layout::{PagePlacement, Rect};
use crate::marks::{MarksConfig, generate_marks};
use crate::types::{PrinterMarks, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::HashMap;

use super::page_numbers::render_page_numbers;
use super::xobject::create_page_xobject;

// =============================================================================
// Public API
// =============================================================================

/// Render an imposed output page.
///
/// This is a standalone function that can be used to create custom imposed pages.
/// For standard imposition workflows, use `impose()` instead.
///
/// # Arguments
/// * `output` - The output document
/// * `source` - The source document containing the pages
/// * `source_page_ids` - Object IDs of all source pages
/// * `placements` - Page placements for this output page
/// * `sheet_width_pt` - Output page width in points
/// * `sheet_height_pt` - Output page height in points
/// * `parent_pages_id` - The parent Pages object ID
/// * `marks` - Printer's marks configuration
/// * `leaf_bounds` - The leaf area bounds (for marks)
/// * `grid_cols` - Number of spread columns
/// * `grid_rows` - Number of spread rows
/// * `add_page_numbers` - Whether to add page numbers
/// * `page_number_start` - Starting page number
#[allow(clippy::too_many_arguments)]
pub fn render_imposed_page(
    output: &mut Document,
    source: &Document,
    source_page_ids: &[ObjectId],
    placements: &[PagePlacement],
    sheet_width_pt: f32,
    sheet_height_pt: f32,
    parent_pages_id: ObjectId,
    marks: &PrinterMarks,
    leaf_bounds: &Rect,
    grid_cols: usize,
    grid_rows: usize,
    add_page_numbers: bool,
    page_number_start: usize,
) -> Result<ObjectId> {
    // Create page dictionary
    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("Parent", Object::Reference(parent_pages_id));
    page_dict.set(
        "MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Real(sheet_width_pt),
            Object::Real(sheet_height_pt),
        ]),
    );

    let mut content_ops = Vec::new();
    let mut xobjects = Dictionary::new();
    let mut fonts = Dictionary::new();
    let mut xobject_cache: HashMap<ObjectId, ObjectId> = HashMap::new();
    // Render each page placement
    for (idx, placement) in placements.iter().enumerate() {
        if let Some(source_idx) = placement.source_page {
            if source_idx < source_page_ids.len() {
                let source_page_id = source_page_ids[source_idx];
                let xobject_name = format!("P{}", idx);

                let xobject_id =
                    create_page_xobject(output, source, source_page_id, &mut xobject_cache)?;
                xobjects.set(xobject_name.as_bytes(), Object::Reference(xobject_id));

                content_ops.push(generate_placement_command(
                    &xobject_name,
                    &placement.content_rect,
                    placement.scale,
                    placement.rotation_degrees,
                ));
            }
        }
    }

    // Generate printer's marks
    if marks.any_enabled() {
        let marks_config = MarksConfig::simple(grid_cols, grid_rows, leaf_bounds);
        content_ops.push(generate_marks(marks, &marks_config, None));
    }

    // Add page numbers
    if add_page_numbers {
        let (font_ops, font_id) = render_page_numbers(
            output,
            placements,
            page_number_start,
        );
        content_ops.push(font_ops);
        fonts.set("F1", Object::Reference(font_id));
    }

    // Set up resources
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

/// Generate the PDF content stream command to place a page.
fn generate_placement_command(
    xobject_name: &str,
    rect: &Rect,
    scale: f32,
    rotation_degrees: f32,
) -> String {
    if rotation_degrees.abs() > 0.1 {
        // 180° rotation
        let rot_x = rect.x + rect.width;
        let rot_y = rect.y + rect.height;
        format!(
            "q {} 0 0 {} {} {} cm /{} Do Q\n",
            -scale, -scale, rot_x, rot_y, xobject_name
        )
    } else {
        format!(
            "q {} 0 0 {} {} {} cm /{} Do Q\n",
            scale, scale, rect.x, rect.y, xobject_name
        )
    }
}

