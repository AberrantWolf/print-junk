//! Sheet rendering for imposition using spread-based layout

use super::page_source::{PageSource, XObjectCache};
use crate::layout::{
    ArrangementConfig, PagePlacement, SpreadCutEdges, SpreadSheetLayout,
    calculate_spread_placements,
};
use crate::marks::{MarksConfig, MarksContext, generate_marks};
use crate::options::ImpositionOptions;
use crate::render::render_page_numbers;
use crate::types::Result;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

// =============================================================================
// Sheet Content Generation
// =============================================================================

/// Generated content for one side of an imposed sheet
pub(crate) struct SheetContent {
    /// PDF content stream operations
    pub content: String,
    /// `XObject` references used in the content
    pub xobjects: Dictionary,
    /// Font references used in the content
    pub fonts: Dictionary,
}

/// Generate the content for one side of a sheet without creating a page.
///
/// This is the reusable core of sheet rendering. It produces the PDF content
/// stream, `XObject` references, and font references that can be assembled
/// into either a page (normal path) or a Form `XObject` (cascade path).
pub(crate) fn generate_sheet_content(
    output: &mut Document,
    page_source: &PageSource,
    layout: &SpreadSheetLayout,
    cut_edges: &[SpreadCutEdges],
    options: &ImpositionOptions,
    signature_index: usize,
    total_signatures: usize,
    sheet_in_signature: usize,
    xobject_cache: &mut XObjectCache,
) -> Result<SheetContent> {
    let mut content_ops = Vec::new();
    let mut xobjects = Dictionary::new();
    let mut fonts = Dictionary::new();

    // Calculate page placements from spreads
    let source_dimensions = page_source.all_dimensions();
    let placements = calculate_spread_placements(
        &layout.spreads,
        cut_edges,
        &source_dimensions,
        &options.margins.leaf,
        options.scaling_mode,
        layout.side,
    );

    // Render each page placement
    for (idx, placement) in placements.iter().enumerate() {
        if let Some(source_idx) = placement.source_page
            && source_idx < page_source.len()
            && let Some(xobject_id) =
                page_source.create_xobject(output, source_idx, xobject_cache)?
        {
            let xobject_name = format!("P{idx}");
            xobjects.set(xobject_name.as_bytes(), Object::Reference(xobject_id));
            content_ops.push(generate_placement_cmd(&xobject_name, placement));
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
            sheet_side: layout.side,
            sheet_in_signature,
        };
        content_ops.push(generate_marks(
            options.marks,
            &marks_config,
            Some(&marks_ctx),
            options.interior_marks_appearance,
            options.exterior_marks_appearance,
        ));
    }

    // Add page numbers
    if options.add_page_numbers {
        let (font_ops, font_id) =
            render_page_numbers(output, &placements, options.page_number_start);
        content_ops.push(font_ops);
        fonts.set("F1", Object::Reference(font_id));
    }

    Ok(SheetContent {
        content: content_ops.join(""),
        xobjects,
        fonts,
    })
}

// =============================================================================
// Spread-Based Sheet Rendering
// =============================================================================

/// Render one side of a sheet using spread-based layout
pub(crate) fn render_sheet_spreads(
    output: &mut Document,
    page_source: &PageSource,
    layout: &SpreadSheetLayout,
    cut_edges: &[SpreadCutEdges],
    sheet_width_pt: f32,
    sheet_height_pt: f32,
    parent_pages_id: ObjectId,
    options: &ImpositionOptions,
    signature_index: usize,
    total_signatures: usize,
    sheet_in_signature: usize,
    xobject_cache: &mut XObjectCache,
) -> Result<ObjectId> {
    let sheet = generate_sheet_content(
        output,
        page_source,
        layout,
        cut_edges,
        options,
        signature_index,
        total_signatures,
        sheet_in_signature,
        xobject_cache,
    )?;

    let mut page_dict = create_page_dict(parent_pages_id, sheet_width_pt, sheet_height_pt);

    // Build resources
    let mut resources = Dictionary::new();
    resources.set("XObject", Object::Dictionary(sheet.xobjects));
    if !sheet.fonts.is_empty() {
        resources.set("Font", Object::Dictionary(sheet.fonts));
    }

    // Create content stream
    let content_id = output.add_object(Stream::new(Dictionary::new(), sheet.content.into_bytes()));

    page_dict.set("Contents", Object::Reference(content_id));
    page_dict.set("Resources", Object::Dictionary(resources));

    Ok(output.add_object(page_dict))
}

/// Create a Form `XObject` from sheet content, suitable for embedding in a cascade page.
pub(crate) fn create_sheet_xobject(
    output: &mut Document,
    sheet: SheetContent,
    width_pt: f32,
    height_pt: f32,
) -> ObjectId {
    let mut resources = Dictionary::new();
    resources.set("XObject", Object::Dictionary(sheet.xobjects));
    if !sheet.fonts.is_empty() {
        resources.set("Font", Object::Dictionary(sheet.fonts));
    }

    let mut xobject_dict = Dictionary::new();
    xobject_dict.set("Type", Object::Name(b"XObject".to_vec()));
    xobject_dict.set("Subtype", Object::Name(b"Form".to_vec()));
    xobject_dict.set(
        "BBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Real(width_pt),
            Object::Real(height_pt),
        ]),
    );
    xobject_dict.set("Resources", Object::Dictionary(resources));

    let stream = Stream::new(xobject_dict, sheet.content.into_bytes());
    output.add_object(stream)
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

/// Generate PDF command to place an `XObject`
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
