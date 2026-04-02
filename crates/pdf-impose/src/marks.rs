//! Printer's marks rendering for imposed pages
//!
//! This module generates PDF content stream operations for various printer's marks:
//! fold lines, crop marks, registration marks, etc.
//!
//! Marks are rendered per-leaf (the folded/trimmed unit), not per-page.

use crate::constants::{
    BEZIER_CIRCLE_FACTOR, CROP_MARK_GAP, CROP_MARK_LENGTH, CROP_MARK_WIDTH, CUT_LINE_WIDTH,
    FOLD_LINE_WIDTH, REGISTRATION_MARK_SIZE, REGISTRATION_MARK_WIDTH, SCISSORS_SIZE,
};
use crate::types::PrinterMarks;

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for rendering marks on an imposed sheet.
///
/// The layout hierarchy is:
/// - Sheet: The entire output page (e.g., Letter, A3)
/// - Leaf area: The region inside sheet margins where content is placed
/// - Cells: Individual page positions within the leaf area (arranged in grid)
pub struct MarksConfig {
    /// Number of columns in the page grid
    pub cols: usize,
    /// Number of rows in the page grid
    pub rows: usize,
    /// Width of each cell (page position) in points
    pub cell_width: f32,
    /// Height of each cell (page position) in points
    pub cell_height: f32,
    /// Left edge of the leaf area in points (after sheet margins)
    pub leaf_left: f32,
    /// Bottom edge of the leaf area in points (after sheet margins)
    pub leaf_bottom: f32,
    /// Right edge of the leaf area in points
    pub leaf_right: f32,
    /// Top edge of the leaf area in points
    pub leaf_top: f32,
    /// Content boundaries for each cell (for trim marks)
    /// Stored in row-major order: row 0 col 0, row 0 col 1, ..., row 1 col 0, ...
    pub content_bounds: Vec<ContentBounds>,
    /// Column indices where vertical cuts occur (e.g., [1] for octavo center cut)
    pub vertical_cuts: Vec<usize>,
    /// Row indices where horizontal cuts occur (cuts between row N and row N+1)
    pub horizontal_cuts: Vec<usize>,
}

/// Bounds of actual content within a cell
#[derive(Clone, Copy, Default)]
pub struct ContentBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ContentBounds {
    /// Check if bounds are valid (positive area)
    pub fn is_valid(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn top(&self) -> f32 {
        self.y + self.height
    }
}

// =============================================================================
// Main Entry Point
// =============================================================================

/// Generate all printer's marks as PDF content stream operations
pub fn generate_marks(marks: &PrinterMarks, config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Save graphics state and set default stroke color
    ops.push_str("q\n0 0 0 RG\n");

    if marks.fold_lines {
        ops.push_str(&generate_fold_lines(config));
    }

    if marks.cut_lines {
        ops.push_str(&generate_cut_lines(config));
    }

    if marks.trim_marks {
        ops.push_str(&generate_trim_marks(config));
    }

    if marks.crop_marks {
        ops.push_str(&generate_crop_marks(config));
    }

    if marks.registration_marks {
        ops.push_str(&generate_registration_marks(config));
    }

    // Restore graphics state
    ops.push_str("Q\n");

    ops
}

// =============================================================================
// Fold Lines
// =============================================================================

/// Generate fold lines (dashed lines at fold positions)
fn generate_fold_lines(config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Set line properties for fold lines (dashed)
    ops.push_str(&format!("{} w\n[6 3] 0 d\n", FOLD_LINE_WIDTH));

    // Vertical fold lines (between columns, skipping cut positions)
    for col in 1..config.cols {
        if config.vertical_cuts.contains(&(col - 1)) {
            continue; // This boundary is a cut, not a fold
        }
        let x = config.leaf_left + col as f32 * config.cell_width;
        ops.push_str(&draw_line(x, config.leaf_bottom, x, config.leaf_top));
    }

    // Reset to solid line
    ops.push_str("[] 0 d\n");

    ops
}

// =============================================================================
// Cut Lines
// =============================================================================

/// Generate cut lines (solid lines with scissors at cut positions)
fn generate_cut_lines(config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Set line properties for cut lines (solid)
    ops.push_str(&format!("{} w\n[] 0 d\n", CUT_LINE_WIDTH));

    // Horizontal cut lines from config
    for &cut_row in &config.horizontal_cuts {
        let y = config.leaf_bottom + (cut_row + 1) as f32 * config.cell_height;
        ops.push_str(&draw_line(config.leaf_left, y, config.leaf_right, y));
        ops.push_str(&draw_scissors_horizontal(
            config.leaf_left - SCISSORS_SIZE - 3.0,
            y,
        ));
    }

    // Vertical cut lines from config
    for &cut_col in &config.vertical_cuts {
        let x = config.leaf_left + (cut_col + 1) as f32 * config.cell_width;
        ops.push_str(&draw_line(x, config.leaf_bottom, x, config.leaf_top));
        ops.push_str(&draw_scissors_vertical(
            x,
            config.leaf_bottom - SCISSORS_SIZE - 3.0,
        ));
    }

    ops
}

// =============================================================================
// Trim Marks
// =============================================================================

/// Generate trim marks at cut positions.
///
/// Trim marks appear at cell corners that are adjacent to cut lines.
/// According to traditional bookbinding, these indicate where pages will
/// be trimmed after folding. They appear between leaf margins and cut margins.
fn generate_trim_marks(config: &MarksConfig) -> String {
    if config.content_bounds.is_empty() {
        return String::new();
    }

    let mut ops = String::new();
    ops.push_str(&format!("{} w\n[] 0 d\n", CROP_MARK_WIDTH));

    // For each cell, determine which edges are at cut positions and draw
    // trim marks only at those corners
    for row in 0..config.rows {
        for col in 0..config.cols {
            let idx = row * config.cols + col;
            if idx >= config.content_bounds.len() {
                continue;
            }
            let bounds = &config.content_bounds[idx];
            if !bounds.is_valid() {
                continue;
            }

            // Check if this cell has a cut on its right edge
            let cut_right = config.vertical_cuts.contains(&col);
            // Check if this cell has a cut on its left edge
            let cut_left = col > 0 && config.vertical_cuts.contains(&(col - 1));
            // Check if this cell has a cut on its bottom edge (cut between this row and row below)
            // Note: row 0 is top, so "bottom" of row 0 is the cut at index 0
            let cut_bottom = config.horizontal_cuts.contains(&row);
            // Check if this cell has a cut on its top edge
            let cut_top = row > 0 && config.horizontal_cuts.contains(&(row - 1));

            // Draw trim marks at corners adjacent to cuts
            if cut_top || cut_left {
                ops.push_str(&draw_corner_mark(bounds.x, bounds.top(), -1.0, 1.0));
            }
            if cut_top || cut_right {
                ops.push_str(&draw_corner_mark(bounds.right(), bounds.top(), 1.0, 1.0));
            }
            if cut_bottom || cut_left {
                ops.push_str(&draw_corner_mark(bounds.x, bounds.y, -1.0, -1.0));
            }
            if cut_bottom || cut_right {
                ops.push_str(&draw_corner_mark(bounds.right(), bounds.y, 1.0, -1.0));
            }
        }
    }

    ops
}

// =============================================================================
// Crop Marks
// =============================================================================

/// Generate crop marks (L-shaped marks at corners of the leaf area)
fn generate_crop_marks(config: &MarksConfig) -> String {
    let mut ops = String::new();
    ops.push_str(&format!("{} w\n[] 0 d\n", CROP_MARK_WIDTH));
    ops.push_str(&draw_corner_marks(
        config.leaf_left,
        config.leaf_bottom,
        config.leaf_right,
        config.leaf_top,
    ));
    ops
}

/// Draw L-shaped corner marks at all four corners of a rectangle
fn draw_corner_marks(left: f32, bottom: f32, right: f32, top: f32) -> String {
    let mut ops = String::new();
    ops.push_str(&draw_corner_mark(left, top, -1.0, 1.0));
    ops.push_str(&draw_corner_mark(right, top, 1.0, 1.0));
    ops.push_str(&draw_corner_mark(left, bottom, -1.0, -1.0));
    ops.push_str(&draw_corner_mark(right, bottom, 1.0, -1.0));
    ops
}

/// Draw L-shaped mark at a corner point.
///
/// `x_sign` and `y_sign` control the direction of the mark arms:
/// - Top-left: (-1.0, 1.0)
/// - Top-right: (1.0, 1.0)
/// - Bottom-left: (-1.0, -1.0)
/// - Bottom-right: (1.0, -1.0)
fn draw_corner_mark(x: f32, y: f32, x_sign: f32, y_sign: f32) -> String {
    let mut ops = String::new();
    ops.push_str(&draw_line(
        x,
        y + y_sign * CROP_MARK_GAP,
        x,
        y + y_sign * (CROP_MARK_GAP + CROP_MARK_LENGTH),
    ));
    ops.push_str(&draw_line(
        x + x_sign * CROP_MARK_GAP,
        y,
        x + x_sign * (CROP_MARK_GAP + CROP_MARK_LENGTH),
        y,
    ));
    ops
}

// =============================================================================
// Registration Marks
// =============================================================================

/// Generate registration marks (crosshair circles at midpoints of leaf edges)
fn generate_registration_marks(config: &MarksConfig) -> String {
    let mut ops = String::new();
    ops.push_str(&format!("{} w\n", REGISTRATION_MARK_WIDTH));

    let offset = CROP_MARK_GAP + REGISTRATION_MARK_SIZE;
    let half_size = REGISTRATION_MARK_SIZE / 2.0;

    let mid_x = (config.leaf_left + config.leaf_right) / 2.0;
    let mid_y = (config.leaf_top + config.leaf_bottom) / 2.0;

    // Draw at center of each edge
    let positions = [
        (mid_x, config.leaf_top + offset),    // Top center
        (mid_x, config.leaf_bottom - offset), // Bottom center
        (config.leaf_left - offset, mid_y),   // Left center
        (config.leaf_right + offset, mid_y),  // Right center
    ];

    for (x, y) in positions {
        ops.push_str(&draw_registration_mark(x, y, half_size));
    }

    ops
}

/// Draw a single registration mark (crosshair with circle)
fn draw_registration_mark(cx: f32, cy: f32, half_size: f32) -> String {
    let mut ops = String::new();

    // Crosshair lines
    ops.push_str(&draw_line(cx - half_size, cy, cx + half_size, cy));
    ops.push_str(&draw_line(cx, cy - half_size, cx, cy + half_size));

    // Circle (slightly smaller than crosshair)
    ops.push_str(&draw_circle(cx, cy, half_size * 0.7));

    ops
}

// =============================================================================
// Scissors Symbol
// =============================================================================

/// Draw scissors symbol pointing right (for horizontal cut lines)
fn draw_scissors_horizontal(x: f32, y: f32) -> String {
    let half = SCISSORS_SIZE / 2.0;
    let r = half * 0.4; // finger hole radius

    let mut ops = String::new();
    ops.push_str("q\n0.3 w\n");

    // Upper finger hole
    let cx1 = x + half * 0.3;
    let cy1 = y + half * 0.5;
    ops.push_str(&draw_circle(cx1, cy1, r));

    // Lower finger hole
    let cx2 = x + half * 0.3;
    let cy2 = y - half * 0.5;
    ops.push_str(&draw_circle(cx2, cy2, r));

    // Blades extending to the right
    ops.push_str(&draw_line(
        cx1 + r,
        cy1 - r * 0.5,
        x + SCISSORS_SIZE,
        y + 1.0,
    ));
    ops.push_str(&draw_line(
        cx2 + r,
        cy2 + r * 0.5,
        x + SCISSORS_SIZE,
        y - 1.0,
    ));

    ops.push_str("Q\n");
    ops
}

/// Draw scissors symbol pointing up (for vertical cut lines)
fn draw_scissors_vertical(x: f32, y: f32) -> String {
    let half = SCISSORS_SIZE / 2.0;
    let r = half * 0.4;

    let mut ops = String::new();
    ops.push_str("q\n0.3 w\n");

    // Left finger hole
    let cx1 = x - half * 0.5;
    let cy1 = y + half * 0.3;
    ops.push_str(&draw_circle(cx1, cy1, r));

    // Right finger hole
    let cx2 = x + half * 0.5;
    let cy2 = y + half * 0.3;
    ops.push_str(&draw_circle(cx2, cy2, r));

    // Blades extending upward
    ops.push_str(&draw_line(
        cx1 + r * 0.5,
        cy1 + r,
        x - 1.0,
        y + SCISSORS_SIZE,
    ));
    ops.push_str(&draw_line(
        cx2 - r * 0.5,
        cy2 + r,
        x + 1.0,
        y + SCISSORS_SIZE,
    ));

    ops.push_str("Q\n");
    ops
}

// =============================================================================
// PDF Drawing Primitives
// =============================================================================

/// Draw a line from (x1, y1) to (x2, y2)
fn draw_line(x1: f32, y1: f32, x2: f32, y2: f32) -> String {
    format!("{} {} m {} {} l S\n", x1, y1, x2, y2)
}

/// Draw a circle at (cx, cy) with given radius using Bezier curves
fn draw_circle(cx: f32, cy: f32, r: f32) -> String {
    let k = r * BEZIER_CIRCLE_FACTOR;

    format!(
        "{} {} m\n\
         {} {} {} {} {} {} c\n\
         {} {} {} {} {} {} c\n\
         {} {} {} {} {} {} c\n\
         {} {} {} {} {} {} c\n\
         S\n",
        cx + r,
        cy, // start at right
        cx + r,
        cy + k,
        cx + k,
        cy + r,
        cx,
        cy + r, // to top
        cx - k,
        cy + r,
        cx - r,
        cy + k,
        cx - r,
        cy, // to left
        cx - r,
        cy - k,
        cx - k,
        cy - r,
        cx,
        cy - r, // to bottom
        cx + k,
        cy - r,
        cx + r,
        cy - k,
        cx + r,
        cy, // back to start
    )
}
