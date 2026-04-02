//! Printer's marks rendering for imposed pages
//!
//! This module generates PDF content stream operations for various printer's marks:
//! fold lines, crop marks, registration marks, etc.
//!
//! Marks are rendered per-leaf (the folded/trimmed unit), not per-page.

use crate::constants::{
    BEZIER_CIRCLE_FACTOR, COLLATION_MARK_HEIGHT, COLLATION_MARK_WIDTH, CROP_MARK_GAP,
    CROP_MARK_LENGTH, CROP_MARK_WIDTH, CUT_LINE_WIDTH, FOLD_LINE_WIDTH, REGISTRATION_MARK_SIZE,
    REGISTRATION_MARK_WIDTH, SCISSORS_SIZE, SEWING_MARK_LENGTH, SEWING_MARK_WIDTH,
};
use crate::constants::mm_to_pt;
use crate::types::{BoundaryType, BindingType, PrinterMarks, SewingConfig};

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
    /// Column indices with internal vertical boundaries
    pub vertical_boundaries: Vec<usize>,
    /// Row indices with internal horizontal boundaries
    pub horizontal_boundaries: Vec<usize>,
    /// Whether internal boundaries are folds or cuts (derived from binding type)
    pub boundary_type: BoundaryType,
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

/// Per-sheet rendering context for marks that depend on binding/signature info.
///
/// Separated from `MarksConfig` because this carries runtime context that
/// varies per sheet, while `MarksConfig` is purely geometric.
pub struct MarksContext {
    /// Binding type (for auto-filtering inapplicable marks)
    pub binding_type: BindingType,
    /// Signature index (0-based, for collation mark staircase positioning)
    pub signature_index: usize,
    /// Total number of signatures (for collation mark step sizing)
    pub total_signatures: usize,
    /// Sewing station configuration
    pub sewing_config: SewingConfig,
}

// =============================================================================
// Main Entry Point
// =============================================================================

/// Generate all printer's marks as PDF content stream operations.
///
/// Pass `ctx` as `None` when calling from the standalone render API (no binding context).
/// Sewing and collation marks require a context and are silently skipped without one.
pub fn generate_marks(marks: &PrinterMarks, config: &MarksConfig, ctx: Option<&MarksContext>) -> String {
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

    if let Some(ctx) = ctx {
        if marks.sewing_marks {
            ops.push_str(&generate_sewing_marks(config, ctx));
        }

        if marks.collation_marks {
            ops.push_str(&generate_collation_marks(config, ctx));
        }
    }

    // Restore graphics state
    ops.push_str("Q\n");

    ops
}

// =============================================================================
// Fold Lines
// =============================================================================

/// Generate fold lines (dashed lines at fold positions).
///
/// Fold lines are drawn at all internal boundaries when `boundary_type == Fold`.
/// When `boundary_type == Cut`, there are no folds to draw.
fn generate_fold_lines(config: &MarksConfig) -> String {
    if config.boundary_type != BoundaryType::Fold {
        return String::new();
    }

    let mut ops = String::new();
    ops.push_str(&format!("{} w\n[6 3] 0 d\n", FOLD_LINE_WIDTH));

    // Vertical fold lines
    for &col in &config.vertical_boundaries {
        let x = config.leaf_left + (col + 1) as f32 * config.cell_width;
        ops.push_str(&draw_line(x, config.leaf_bottom, x, config.leaf_top));
    }

    // Horizontal fold lines
    for &row in &config.horizontal_boundaries {
        let y = config.leaf_bottom + (row + 1) as f32 * config.cell_height;
        ops.push_str(&draw_line(config.leaf_left, y, config.leaf_right, y));
    }

    // Reset to solid line
    ops.push_str("[] 0 d\n");

    ops
}

// =============================================================================
// Cut Lines
// =============================================================================

/// Generate cut lines (solid lines with scissors at cut positions).
///
/// Cut lines are drawn at all internal boundaries when `boundary_type == Cut`.
/// When `boundary_type == Fold`, there are no cuts to draw.
fn generate_cut_lines(config: &MarksConfig) -> String {
    if config.boundary_type != BoundaryType::Cut {
        return String::new();
    }

    let mut ops = String::new();
    ops.push_str(&format!("{} w\n[] 0 d\n", CUT_LINE_WIDTH));

    // Horizontal cut lines
    for &row in &config.horizontal_boundaries {
        let y = config.leaf_bottom + (row + 1) as f32 * config.cell_height;
        ops.push_str(&draw_line(config.leaf_left, y, config.leaf_right, y));
        ops.push_str(&draw_scissors_horizontal(
            config.leaf_left - SCISSORS_SIZE - 3.0,
            y,
        ));
    }

    // Vertical cut lines
    for &col in &config.vertical_boundaries {
        let x = config.leaf_left + (col + 1) as f32 * config.cell_width;
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
/// Trim marks appear at cell corners adjacent to cut lines, indicating where
/// pages will be trimmed. Only drawn when `boundary_type == Cut` — fold
/// boundaries don't need trim guidance.
fn generate_trim_marks(config: &MarksConfig) -> String {
    if config.boundary_type != BoundaryType::Cut || config.content_bounds.is_empty() {
        return String::new();
    }

    let mut ops = String::new();
    ops.push_str(&format!("{} w\n[] 0 d\n", CROP_MARK_WIDTH));

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

            let cut_right = config.vertical_boundaries.contains(&col);
            let cut_left = col > 0 && config.vertical_boundaries.contains(&(col - 1));
            let cut_bottom = config.horizontal_boundaries.contains(&row);
            let cut_top = row > 0 && config.horizontal_boundaries.contains(&(row - 1));

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
// Sewing Station Marks
// =============================================================================

/// Generate sewing station marks along the spine fold.
///
/// Auto-filters: only renders for signature/case binding.
/// Draws small horizontal tick marks at kettle stitch and sewing station positions
/// along the spine fold of each spread column.
fn generate_sewing_marks(config: &MarksConfig, ctx: &MarksContext) -> String {
    if !ctx.binding_type.uses_signatures() {
        return String::new();
    }

    let mut ops = String::new();
    ops.push_str(&format!("{} w\n[] 0 d\n", SEWING_MARK_WIDTH));

    let kettle_offset_pt = mm_to_pt(ctx.sewing_config.kettle_offset_mm);
    let half_mark = SEWING_MARK_LENGTH / 2.0;

    // The spine fold is at the center of each spread column.
    // Each spread column spans 2 cells (verso + recto), but in our grid model
    // cols = number of spread columns. The spine of spread column i is at the
    // center of that column's width in the leaf.
    let spread_col_width = (config.leaf_right - config.leaf_left)
        / config.cols.max(1) as f32;

    for col in 0..config.cols {
        let spine_x = config.leaf_left + (col as f32 + 0.5) * spread_col_width;

        // For multi-row arrangements, draw sewing marks in each row
        for row in 0..config.rows {
            let cell_bottom = config.leaf_bottom + row as f32 * config.cell_height;
            let cell_top = cell_bottom + config.cell_height;

            // Kettle stitch positions (near head and tail)
            let kettle_top = cell_top - kettle_offset_pt;
            let kettle_bottom = cell_bottom + kettle_offset_pt;

            // Draw kettle stitch marks
            ops.push_str(&draw_line(
                spine_x - half_mark, kettle_top, spine_x + half_mark, kettle_top,
            ));
            ops.push_str(&draw_line(
                spine_x - half_mark, kettle_bottom, spine_x + half_mark, kettle_bottom,
            ));

            // Evenly spaced sewing stations between kettle positions
            let station_count = ctx.sewing_config.station_count;
            if station_count > 0 {
                let span = kettle_top - kettle_bottom;
                let step = span / (station_count + 1) as f32;

                for i in 1..=station_count {
                    let y = kettle_bottom + i as f32 * step;
                    ops.push_str(&draw_line(
                        spine_x - half_mark, y, spine_x + half_mark, y,
                    ));
                }
            }
        }
    }

    ops
}

// =============================================================================
// Collation Marks (Back Marks)
// =============================================================================

/// Generate collation marks (staircase pattern on spine for signature ordering).
///
/// Auto-filters: only renders for signature/case binding with multiple signatures.
/// Draws a small filled rectangle on the spine fold edge that steps down per
/// signature, forming a visible staircase when signatures are stacked in order.
fn generate_collation_marks(config: &MarksConfig, ctx: &MarksContext) -> String {
    if !ctx.binding_type.uses_signatures() || ctx.total_signatures <= 1 {
        return String::new();
    }

    let mut ops = String::new();
    // Set fill color to black
    ops.push_str("0 0 0 rg\n");

    let leaf_height = config.leaf_top - config.leaf_bottom;

    // Step size: distribute marks across the spine height
    let usable_height = leaf_height - COLLATION_MARK_HEIGHT;
    let step = if ctx.total_signatures > 1 {
        usable_height / (ctx.total_signatures - 1) as f32
    } else {
        0.0
    };

    // Y position steps down from head (top) toward tail (bottom)
    let mark_y = config.leaf_top - COLLATION_MARK_HEIGHT
        - ctx.signature_index as f32 * step;

    // Draw on the spine fold (center of each spread column)
    let spread_col_width = (config.leaf_right - config.leaf_left)
        / config.cols.max(1) as f32;

    for col in 0..config.cols {
        let spine_x = config.leaf_left + (col as f32 + 0.5) * spread_col_width;
        // Rectangle centered on the spine
        let rect_x = spine_x - COLLATION_MARK_WIDTH / 2.0;
        ops.push_str(&format!(
            "{} {} {} {} re f\n",
            rect_x, mark_y, COLLATION_MARK_WIDTH, COLLATION_MARK_HEIGHT
        ));
    }

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
