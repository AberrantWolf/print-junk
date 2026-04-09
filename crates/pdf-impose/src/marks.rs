//! Printer's marks rendering for imposed pages
//!
//! This module generates PDF content stream operations for various printer's marks:
//! fold lines, crop marks, registration marks, etc.
//!
//! All mark positions are derived from actual spread layout data via
//! `MarksConfig::from_layout()`, ensuring marks always align with page content.

use std::fmt::Write;

use crate::constants::mm_to_pt;
use crate::constants::{
    BEZIER_CIRCLE_FACTOR, COLLATION_MARK_HEIGHT, COLLATION_MARK_WIDTH, CROP_MARK_GAP,
    CROP_MARK_LENGTH, CROP_MARK_WIDTH, FOLD_LINE_WIDTH, REGISTRATION_MARK_SIZE,
    REGISTRATION_MARK_WIDTH, SEWING_MARK_LENGTH, SEWING_MARK_WIDTH,
};
use crate::layout::{ArrangementConfig, Rect, SheetSide, SpreadPosition};
use crate::types::{BindingType, PrinterMarks, SewingConfig};

// =============================================================================
// Configuration
// =============================================================================

/// Spine fold position for one spread column, with per-row vertical spans.
pub struct SpinePosition {
    /// X coordinate of the spine fold
    pub x: f32,
    /// (bottom, top) vertical span for each row in this column
    pub rows: Vec<(f32, f32)>,
}

/// Configuration for rendering marks on an imposed sheet.
///
/// All positions are pre-computed from actual spread layout data.
/// This ensures marks are always aligned with page content — the layout
/// system is the single source of truth for sheet geometry.
pub struct MarksConfig {
    /// Spine fold positions per spread column (with per-row vertical spans)
    pub spine_positions: Vec<SpinePosition>,

    /// X coordinates of vertical inter-spread boundaries (midpoint of gap)
    pub vertical_boundary_xs: Vec<f32>,
    /// Y coordinates of horizontal inter-spread boundaries (midpoint of gap)
    pub horizontal_boundary_ys: Vec<f32>,

    /// Left edge of the leaf area in points (after sheet margins)
    pub leaf_left: f32,
    /// Bottom edge of the leaf area in points (after sheet margins)
    pub leaf_bottom: f32,
    /// Right edge of the leaf area in points
    pub leaf_right: f32,
    /// Top edge of the leaf area in points
    pub leaf_top: f32,

    /// Trim allowance in points (gap at inter-spread boundaries for guillotine trimming)
    pub trim_allowance_pt: f32,
}

impl MarksConfig {
    /// Build marks configuration from actual spread layout data.
    ///
    /// Derives all positions from the spread positions computed by the layout
    /// system, ensuring marks always align with page content.
    pub fn from_layout(
        spreads: &[SpreadPosition],
        config: &ArrangementConfig,
        leaf_bounds: &Rect,
        trim_allowance_pt: f32,
    ) -> Self {
        let cols = config.cols;
        let rows = config.rows;

        // Build spine positions from actual spread centers
        let mut spine_positions = Vec::with_capacity(cols);
        for col in 0..cols {
            // Find any spread in this column to get the spine X
            // All spreads in the same column have the same X origin and width
            let col_spread = spreads.iter().find(|s| s.spread_index % cols == col);
            let spine_x = col_spread.map_or(leaf_bounds.x + leaf_bounds.width / 2.0, |s| {
                s.origin.x + s.width / 2.0
            });

            // Collect per-row vertical spans for this column
            let mut row_spans = Vec::with_capacity(rows);
            for row in 0..rows {
                let spread_idx = row * cols + col;
                if let Some(s) = spreads.iter().find(|s| s.spread_index == spread_idx) {
                    row_spans.push((s.origin.y, s.origin.y + s.height));
                }
            }

            spine_positions.push(SpinePosition {
                x: spine_x,
                rows: row_spans,
            });
        }

        // Vertical boundary positions: midpoint between adjacent columns
        let mut vertical_boundary_xs = Vec::new();
        for col in 0..(cols.saturating_sub(1)) {
            let right_spread = spreads.iter().find(|s| s.spread_index % cols == col);
            let left_next = spreads.iter().find(|s| s.spread_index % cols == col + 1);
            if let (Some(r), Some(l)) = (right_spread, left_next) {
                let right_edge = r.origin.x + r.width;
                let left_edge = l.origin.x;
                vertical_boundary_xs.push(f32::midpoint(right_edge, left_edge));
            }
        }

        // Horizontal boundary positions: midpoint between adjacent rows
        let mut horizontal_boundary_ys = Vec::new();
        for row in 0..(rows.saturating_sub(1)) {
            let top_spread = spreads.iter().find(|s| s.spread_index / cols == row);
            let bottom_next = spreads.iter().find(|s| s.spread_index / cols == row + 1);
            if let (Some(t), Some(b)) = (top_spread, bottom_next) {
                let top_edge = t.origin.y + t.height;
                let bottom_edge = b.origin.y;
                horizontal_boundary_ys.push(f32::midpoint(top_edge, bottom_edge));
            }
        }

        Self {
            spine_positions,
            vertical_boundary_xs,
            horizontal_boundary_ys,
            leaf_left: leaf_bounds.x,
            leaf_bottom: leaf_bounds.y,
            leaf_right: leaf_bounds.right(),
            leaf_top: leaf_bounds.top(),
            trim_allowance_pt,
        }
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
    /// Which side of the physical sheet (front = outside when folded, back = inside)
    pub sheet_side: SheetSide,
    /// Sheet position within signature (0 = outermost sheet)
    pub sheet_in_signature: usize,
}

// =============================================================================
// Side-Aware Filtering
// =============================================================================

/// Filter marks to those appropriate for a given sheet side and position.
///
/// Physical rules for signature binding:
/// - Fold lines: front (outside) only — folding guides
/// - Collation marks: front of outermost sheet only — spine stacking verification
/// - Sewing marks: back (inside) only — sew from inside opened signature
/// - Crop/trim/registration: both sides — printer/cutter alignment
fn filter_marks_for_context(marks: PrinterMarks, ctx: &MarksContext) -> PrinterMarks {
    let is_front = ctx.sheet_side == SheetSide::Front;
    let is_outermost = ctx.sheet_in_signature == 0;

    PrinterMarks {
        fold_lines: marks.fold_lines && is_front,
        collation_marks: marks.collation_marks && is_front && is_outermost,
        sewing_marks: marks.sewing_marks && !is_front,
        ..marks
    }
}

// =============================================================================
// Main Entry Point
// =============================================================================

/// Generate all printer's marks as PDF content stream operations.
///
/// Pass `ctx` as `None` when calling from the standalone render API (no binding context).
/// Sewing and collation marks require a context and are silently skipped without one.
pub fn generate_marks(
    marks: PrinterMarks,
    config: &MarksConfig,
    ctx: Option<&MarksContext>,
) -> String {
    // Filter marks based on physical sheet side and position
    let marks = match ctx {
        Some(c) => filter_marks_for_context(marks, c),
        None => marks,
    };

    let mut ops = String::new();

    // Save graphics state and set default stroke color
    ops.push_str("q\n0 0 0 RG\n");

    if marks.fold_lines {
        ops.push_str(&generate_fold_lines(config));
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
/// Draws dashed lines at:
/// - Inter-spread boundaries (horizontal and vertical)
/// - Spine fold within each spread column (vertical, contained within each row)
fn generate_fold_lines(config: &MarksConfig) -> String {
    let mut ops = String::new();
    let _ = write!(ops, "{FOLD_LINE_WIDTH} w\n[6 3] 0 d\n");

    // Vertical fold lines at inter-spread boundaries
    for &x in &config.vertical_boundary_xs {
        ops.push_str(&draw_line(x, config.leaf_bottom, x, config.leaf_top));
    }

    // Horizontal fold lines at inter-spread boundaries
    for &y in &config.horizontal_boundary_ys {
        ops.push_str(&draw_line(config.leaf_left, y, config.leaf_right, y));
    }

    // Spine fold line within each spread column, drawn per-row
    for spine in &config.spine_positions {
        for &(bottom, top) in &spine.rows {
            ops.push_str(&draw_line(spine.x, bottom, spine.x, top));
        }
    }

    // Reset to solid line
    ops.push_str("[] 0 d\n");

    ops
}

// =============================================================================
// Trim Marks (Guillotine Guides)
// =============================================================================

/// Generate trim marks at inter-spread boundaries.
///
/// These are extension lines at the leaf edges showing where internal guillotine
/// cuts should be made. Each boundary produces a pair of parallel lines (one per
/// edge of the trim allowance gap) extending outward from the leaf into the
/// sheet margin area.
///
/// Only drawn when there are inter-spread boundaries (quarto, octavo, etc.).
fn generate_trim_marks(config: &MarksConfig) -> String {
    if config.horizontal_boundary_ys.is_empty() && config.vertical_boundary_xs.is_empty() {
        return String::new();
    }

    let mut ops = String::new();
    let _ = write!(ops, "{CROP_MARK_WIDTH} w\n[] 0 d\n");

    let half_gap = config.trim_allowance_pt / 2.0;

    // Horizontal boundaries (between spread rows)
    // Draw horizontal extension lines at left and right leaf edges
    for &center_y in &config.horizontal_boundary_ys {
        let top_edge = center_y + half_gap;
        let bottom_edge = center_y - half_gap;

        // Left side: lines extending left from the leaf edge
        ops.push_str(&draw_trim_extension(config.leaf_left, top_edge, -1.0, 0.0));
        ops.push_str(&draw_trim_extension(
            config.leaf_left,
            bottom_edge,
            -1.0,
            0.0,
        ));

        // Right side: lines extending right from the leaf edge
        ops.push_str(&draw_trim_extension(config.leaf_right, top_edge, 1.0, 0.0));
        ops.push_str(&draw_trim_extension(
            config.leaf_right,
            bottom_edge,
            1.0,
            0.0,
        ));
    }

    // Vertical boundaries (between spread columns)
    // Draw vertical extension lines at top and bottom leaf edges
    for &center_x in &config.vertical_boundary_xs {
        let right_edge = center_x + half_gap;
        let left_edge = center_x - half_gap;

        // Top: lines extending up from the leaf edge
        ops.push_str(&draw_trim_extension(left_edge, config.leaf_top, 0.0, 1.0));
        ops.push_str(&draw_trim_extension(right_edge, config.leaf_top, 0.0, 1.0));

        // Bottom: lines extending down from the leaf edge
        ops.push_str(&draw_trim_extension(
            left_edge,
            config.leaf_bottom,
            0.0,
            -1.0,
        ));
        ops.push_str(&draw_trim_extension(
            right_edge,
            config.leaf_bottom,
            0.0,
            -1.0,
        ));
    }

    ops
}

/// Draw a single trim extension line from a point on the leaf edge outward.
///
/// `dx` and `dy` indicate direction: e.g. (-1, 0) = leftward, (0, 1) = upward.
/// The line starts at GAP offset from the origin and extends for `CROP_MARK_LENGTH`.
fn draw_trim_extension(x: f32, y: f32, dx: f32, dy: f32) -> String {
    draw_line(
        x + dx * CROP_MARK_GAP,
        y + dy * CROP_MARK_GAP,
        x + dx * (CROP_MARK_GAP + CROP_MARK_LENGTH),
        y + dy * (CROP_MARK_GAP + CROP_MARK_LENGTH),
    )
}

// =============================================================================
// Crop Marks
// =============================================================================

/// Generate crop marks (L-shaped marks at corners of the leaf area)
fn generate_crop_marks(config: &MarksConfig) -> String {
    let mut ops = String::new();
    let _ = write!(ops, "{CROP_MARK_WIDTH} w\n[] 0 d\n");
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
    let _ = writeln!(ops, "{REGISTRATION_MARK_WIDTH} w");

    let offset = CROP_MARK_GAP + REGISTRATION_MARK_SIZE;
    let half_size = REGISTRATION_MARK_SIZE / 2.0;

    let mid_x = f32::midpoint(config.leaf_left, config.leaf_right);
    let mid_y = f32::midpoint(config.leaf_top, config.leaf_bottom);

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
    let _ = write!(ops, "{SEWING_MARK_WIDTH} w\n[] 0 d\n");

    let kettle_offset_pt = mm_to_pt(ctx.sewing_config.kettle_offset_mm);
    let half_mark = SEWING_MARK_LENGTH / 2.0;

    for spine in &config.spine_positions {
        for &(cell_bottom, cell_top) in &spine.rows {
            // Kettle stitch positions (near head and tail)
            let kettle_top = cell_top - kettle_offset_pt;
            let kettle_bottom = cell_bottom + kettle_offset_pt;

            // Draw kettle stitch marks
            ops.push_str(&draw_line(
                spine.x - half_mark,
                kettle_top,
                spine.x + half_mark,
                kettle_top,
            ));
            ops.push_str(&draw_line(
                spine.x - half_mark,
                kettle_bottom,
                spine.x + half_mark,
                kettle_bottom,
            ));

            // Evenly spaced sewing stations between kettle positions
            let station_count = ctx.sewing_config.station_count;
            if station_count > 0 {
                let span = kettle_top - kettle_bottom;
                let step = span / (station_count + 1) as f32;

                for i in 1..=station_count {
                    let y = kettle_bottom + i as f32 * step;
                    ops.push_str(&draw_line(spine.x - half_mark, y, spine.x + half_mark, y));
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
    let mark_y = config.leaf_top - COLLATION_MARK_HEIGHT - ctx.signature_index as f32 * step;

    for spine in &config.spine_positions {
        let rect_x = spine.x - COLLATION_MARK_WIDTH / 2.0;
        let _ = writeln!(
            ops,
            "{rect_x} {mark_y} {COLLATION_MARK_WIDTH} {COLLATION_MARK_HEIGHT} re f"
        );
    }

    ops
}

// =============================================================================
// PDF Drawing Primitives
// =============================================================================

/// Draw a line from (x1, y1) to (x2, y2)
fn draw_line(x1: f32, y1: f32, x2: f32, y2: f32) -> String {
    format!("{x1} {y1} m {x2} {y2} l S\n")
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
