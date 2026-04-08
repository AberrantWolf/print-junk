//! Cascade assembly — tiles multiple imposed sheets onto a single larger output page.
//!
//! Each imposed sheet side is rendered into a Form `XObject`, then multiple `XObject`s
//! are placed in a grid on the cascade output page. Front and back sides are
//! mirrored according to the flip axis for correct duplex alignment.

use crate::constants::{CASCADE_CUT_LINE_WIDTH, mm_to_pt};
use crate::types::{CascadeConfig, FlipAxis, SheetMargins};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::fmt::Write;

/// A pair of Form `XObject` IDs representing one imposed sheet (front + back)
pub(crate) struct CascadeCell {
    pub front_xobject: ObjectId,
    pub back_xobject: ObjectId,
}

/// Assemble a batch of cells into one cascade output page (front and back).
///
/// `cells` may be smaller than `cols × rows` for the last batch — empty cells are left blank.
pub(crate) fn render_cascade_page(
    output: &mut Document,
    cells: &[CascadeCell],
    cascade: &CascadeConfig,
    cell_width_pt: f32,
    cell_height_pt: f32,
    cascade_width_pt: f32,
    cascade_height_pt: f32,
    sheet_margins: &SheetMargins,
    parent_pages_id: ObjectId,
) -> (ObjectId, ObjectId) {
    let gap = mm_to_pt(cascade.margin_mm);
    let margin_left = mm_to_pt(sheet_margins.left_mm);
    let margin_bottom = mm_to_pt(sheet_margins.bottom_mm);

    // Generate cut lines (shared between front and back)
    let cut_lines = if cascade.cut_lines {
        generate_cascade_cut_lines(
            cascade.cols,
            cascade.rows,
            cell_width_pt,
            cell_height_pt,
            gap,
            cascade_width_pt,
            cascade_height_pt,
            margin_left,
            margin_bottom,
        )
    } else {
        String::new()
    };

    // Front page
    let front_id = render_cascade_side(
        output,
        cells,
        cascade,
        cell_width_pt,
        cell_height_pt,
        cascade_width_pt,
        cascade_height_pt,
        margin_left,
        margin_bottom,
        gap,
        &cut_lines,
        parent_pages_id,
        false, // front — no flip
    );

    // Back page
    let back_id = render_cascade_side(
        output,
        cells,
        cascade,
        cell_width_pt,
        cell_height_pt,
        cascade_width_pt,
        cascade_height_pt,
        margin_left,
        margin_bottom,
        gap,
        &cut_lines,
        parent_pages_id,
        true, // back — apply flip
    );

    (front_id, back_id)
}

/// Render one side (front or back) of a cascade page.
fn render_cascade_side(
    output: &mut Document,
    cells: &[CascadeCell],
    cascade: &CascadeConfig,
    cell_width_pt: f32,
    cell_height_pt: f32,
    cascade_width_pt: f32,
    cascade_height_pt: f32,
    margin_left: f32,
    margin_bottom: f32,
    gap: f32,
    cut_lines: &str,
    parent_pages_id: ObjectId,
    is_back: bool,
) -> ObjectId {
    let mut content_ops = String::new();
    let mut xobjects = Dictionary::new();

    for (idx, cell) in cells.iter().enumerate() {
        let row = idx / cascade.cols;
        let col = idx % cascade.cols;

        // Apply flip for back side
        let (placed_row, placed_col) = if is_back {
            match cascade.flip_axis {
                FlipAxis::LongEdge => (row, cascade.cols - 1 - col),
                FlipAxis::ShortEdge => (cascade.rows - 1 - row, col),
            }
        } else {
            (row, col)
        };

        let x = margin_left + placed_col as f32 * (cell_width_pt + gap);
        let y = margin_bottom + placed_row as f32 * (cell_height_pt + gap);

        let xobject_id = if is_back {
            cell.back_xobject
        } else {
            cell.front_xobject
        };

        let name = format!("C{idx}");
        xobjects.set(name.as_bytes(), Object::Reference(xobject_id));

        let _ = writeln!(content_ops, "q 1 0 0 1 {x} {y} cm /{name} Do Q");
    }

    // Add cut lines
    content_ops.push_str(cut_lines);

    // Build page
    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("Parent", Object::Reference(parent_pages_id));
    page_dict.set(
        "MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Real(cascade_width_pt),
            Object::Real(cascade_height_pt),
        ]),
    );

    let mut resources = Dictionary::new();
    resources.set("XObject", Object::Dictionary(xobjects));

    let content_id = output.add_object(Stream::new(Dictionary::new(), content_ops.into_bytes()));

    page_dict.set("Contents", Object::Reference(content_id));
    page_dict.set("Resources", Object::Dictionary(resources));

    output.add_object(page_dict)
}

/// Generate PDF content stream operations for cascade cut lines.
///
/// Draws solid lines at the center of each gap between cells, extending
/// to the full sheet dimension.
fn generate_cascade_cut_lines(
    cols: usize,
    rows: usize,
    cell_width_pt: f32,
    cell_height_pt: f32,
    gap: f32,
    total_width_pt: f32,
    total_height_pt: f32,
    offset_x: f32,
    offset_y: f32,
) -> String {
    let mut ops = String::new();
    let _ = writeln!(ops, "q {CASCADE_CUT_LINE_WIDTH} w [] 0 d");

    // Vertical cut lines (between columns)
    for col in 1..cols {
        let x = offset_x + col as f32 * cell_width_pt + (col as f32 - 0.5) * gap;
        let _ = writeln!(ops, "{x} 0 m {x} {total_height_pt} l S");
    }

    // Horizontal cut lines (between rows)
    for row in 1..rows {
        let y = offset_y + row as f32 * cell_height_pt + (row as f32 - 0.5) * gap;
        let _ = writeln!(ops, "0 {y} m {total_width_pt} {y} l S");
    }

    let _ = writeln!(ops, "Q");
    ops
}
