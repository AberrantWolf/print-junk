//! Spread layout calculation - the fundamental imposition unit
//!
//! A spread is the basic building block of book layout: two facing pages
//! (verso on left, recto on right). All imposition arrangements are
//! compositions of spreads:
//!
//! - **Folio**: 1 spread
//! - **Quarto**: 2 spreads stacked vertically (top rotated 180 degrees)
//! - **Octavo**: 4 spreads in 2x2 grid (top row rotated 180 degrees)
//!
//! This module calculates content areas within spreads, applying margins
//! correctly based on edge types (spine, fore-edge, cut lines).

use crate::constants::mm_to_pt;
use crate::types::LeafMargins;

use super::{Point, Rect, SpreadContentAreas, SpreadCutEdges, SpreadPosition};

// =============================================================================
// Spread Content Calculation
// =============================================================================

/// Calculate content areas for both pages in a spread.
///
/// This is the core layout function that determines where page content
/// goes within a spread, accounting for:
/// - Spine margin (gutter between facing pages)
/// - Fore-edge margin (outer edge of pages)
/// - Top/bottom margins
/// - Cut margins (extra space at cut lines)
///
/// # Arguments
/// * `spread_pos` - Position and size of the spread on the sheet
/// * `margins` - Leaf margin configuration
/// * `cut_edges` - Which edges have cut lines (need cut margin)
///
/// # Returns
/// Content rectangles for verso (left) and recto (right) pages.
pub fn calculate_spread_content(
    spread_pos: &SpreadPosition,
    margins: &LeafMargins,
    cut_edges: SpreadCutEdges,
) -> SpreadContentAreas {
    let half_width = spread_pos.width / 2.0;

    // Convert margins to points
    let spine_pt = mm_to_pt(margins.spine_mm);
    let fore_edge_pt = mm_to_pt(margins.fore_edge_mm);
    let top_pt = mm_to_pt(margins.top_mm);
    let bottom_pt = mm_to_pt(margins.bottom_mm);
    let cut_pt = mm_to_pt(margins.cut_mm);

    // Calculate horizontal margins for each page
    // Verso (left page): fore-edge on left, spine on right
    // Recto (right page): spine on left, fore-edge on right
    let verso_left = if cut_edges.left { cut_pt } else { fore_edge_pt };
    let verso_right = spine_pt;
    let recto_left = spine_pt;
    let recto_right = if cut_edges.right {
        cut_pt
    } else {
        fore_edge_pt
    };

    // Calculate vertical margins (add cut margin where there are cuts)
    let top_margin = top_pt + if cut_edges.top { cut_pt } else { 0.0 };
    let bottom_margin = bottom_pt + if cut_edges.bottom { cut_pt } else { 0.0 };

    // Calculate page dimensions
    let page_height = spread_pos.height - top_margin - bottom_margin;
    let verso_width = half_width - verso_left - verso_right;
    let recto_width = half_width - recto_left - recto_right;

    // Calculate page positions
    // In PDF coordinates, y=0 is at the bottom
    let verso_x = spread_pos.origin.x + verso_left;
    let recto_x = spread_pos.origin.x + half_width + recto_left;
    let page_y = spread_pos.origin.y + bottom_margin;

    SpreadContentAreas {
        verso: Rect::new(verso_x, page_y, verso_width, page_height),
        recto: Rect::new(recto_x, page_y, recto_width, page_height),
    }
}

/// Create a spread position for a folio (single spread) arrangement.
///
/// The spread occupies the entire leaf bounds.
pub fn create_folio_spread(leaf_bounds: Rect) -> SpreadPosition {
    SpreadPosition::empty(
        Point::new(leaf_bounds.x, leaf_bounds.y),
        leaf_bounds.width,
        leaf_bounds.height,
        false, // Not rotated
        0,     // First (and only) spread
    )
}

/// Create spread positions for a quarto (2 spreads stacked) arrangement.
///
/// Returns [bottom_spread, top_spread] where top is rotated 180 degrees.
pub fn create_quarto_spreads(leaf_bounds: Rect, cut_gap: f32) -> Vec<SpreadPosition> {
    let half_height = (leaf_bounds.height - cut_gap) / 2.0;

    vec![
        // Bottom spread (not rotated) - index 0
        SpreadPosition::empty(
            Point::new(leaf_bounds.x, leaf_bounds.y),
            leaf_bounds.width,
            half_height,
            false,
            0,
        ),
        // Top spread (rotated 180 degrees) - index 1
        SpreadPosition::empty(
            Point::new(leaf_bounds.x, leaf_bounds.y + half_height + cut_gap),
            leaf_bounds.width,
            half_height,
            true,
            1,
        ),
    ]
}

/// Create spread positions for an octavo (4 spreads in 2x2) arrangement.
///
/// Returns spreads in order: [bottom-left, bottom-right, top-left, top-right]
/// where top row is rotated 180 degrees.
pub fn create_octavo_spreads(
    leaf_bounds: Rect,
    cut_gap_h: f32,
    cut_gap_v: f32,
) -> Vec<SpreadPosition> {
    let half_width = (leaf_bounds.width - cut_gap_v) / 2.0;
    let half_height = (leaf_bounds.height - cut_gap_h) / 2.0;

    vec![
        // Bottom-left (not rotated) - index 0
        SpreadPosition::empty(
            Point::new(leaf_bounds.x, leaf_bounds.y),
            half_width,
            half_height,
            false,
            0,
        ),
        // Bottom-right (not rotated) - index 1
        SpreadPosition::empty(
            Point::new(leaf_bounds.x + half_width + cut_gap_v, leaf_bounds.y),
            half_width,
            half_height,
            false,
            1,
        ),
        // Top-left (rotated 180 degrees) - index 2
        SpreadPosition::empty(
            Point::new(leaf_bounds.x, leaf_bounds.y + half_height + cut_gap_h),
            half_width,
            half_height,
            true,
            2,
        ),
        // Top-right (rotated 180 degrees) - index 3
        SpreadPosition::empty(
            Point::new(
                leaf_bounds.x + half_width + cut_gap_v,
                leaf_bounds.y + half_height + cut_gap_h,
            ),
            half_width,
            half_height,
            true,
            3,
        ),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn default_margins() -> LeafMargins {
        LeafMargins {
            top_mm: 10.0,
            bottom_mm: 10.0,
            fore_edge_mm: 15.0,
            spine_mm: 20.0,
            cut_mm: 5.0,
        }
    }

    #[test]
    fn test_folio_spread_creation() {
        let leaf_bounds = Rect::new(25.0, 25.0, 800.0, 600.0);
        let spread = create_folio_spread(leaf_bounds);

        assert_eq!(spread.origin.x, 25.0);
        assert_eq!(spread.origin.y, 25.0);
        assert_eq!(spread.width, 800.0);
        assert_eq!(spread.height, 600.0);
        assert!(!spread.rotated);
        assert_eq!(spread.spread_index, 0);
    }

    #[test]
    fn test_quarto_spread_creation() {
        let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
        let cut_gap = 10.0;
        let spreads = create_quarto_spreads(leaf_bounds, cut_gap);

        assert_eq!(spreads.len(), 2);

        // Bottom spread
        assert_eq!(spreads[0].origin.y, 0.0);
        assert_eq!(spreads[0].height, 295.0); // (600 - 10) / 2
        assert!(!spreads[0].rotated);

        // Top spread
        assert_eq!(spreads[1].origin.y, 305.0); // 295 + 10
        assert_eq!(spreads[1].height, 295.0);
        assert!(spreads[1].rotated);
    }

    #[test]
    fn test_octavo_spread_creation() {
        let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
        let cut_gap_h = 10.0;
        let cut_gap_v = 10.0;
        let spreads = create_octavo_spreads(leaf_bounds, cut_gap_h, cut_gap_v);

        assert_eq!(spreads.len(), 4);

        let half_width = (800.0 - 10.0) / 2.0; // 395
        let half_height = (600.0 - 10.0) / 2.0; // 295

        // Bottom-left
        assert_eq!(spreads[0].origin.x, 0.0);
        assert_eq!(spreads[0].origin.y, 0.0);
        assert_eq!(spreads[0].width, half_width);
        assert!(!spreads[0].rotated);

        // Bottom-right
        assert_eq!(spreads[1].origin.x, half_width + 10.0);
        assert_eq!(spreads[1].origin.y, 0.0);
        assert!(!spreads[1].rotated);

        // Top-left
        assert_eq!(spreads[2].origin.x, 0.0);
        assert_eq!(spreads[2].origin.y, half_height + 10.0);
        assert!(spreads[2].rotated);

        // Top-right
        assert_eq!(spreads[3].origin.x, half_width + 10.0);
        assert_eq!(spreads[3].origin.y, half_height + 10.0);
        assert!(spreads[3].rotated);
    }

    #[test]
    fn test_spread_content_no_cuts() {
        let spread_pos = SpreadPosition::empty(
            Point::new(0.0, 0.0),
            400.0, // 200 per page
            300.0,
            false,
            0,
        );
        let margins = default_margins();
        let cut_edges = SpreadCutEdges::none();

        let content = calculate_spread_content(&spread_pos, &margins, cut_edges);

        // Verso: x = fore_edge, width = 200 - fore_edge - spine
        let fore_edge_pt = mm_to_pt(15.0);
        let spine_pt = mm_to_pt(20.0);
        let top_pt = mm_to_pt(10.0);
        let bottom_pt = mm_to_pt(10.0);

        assert!((content.verso.x - fore_edge_pt).abs() < 0.1);
        assert!((content.verso.width - (200.0 - fore_edge_pt - spine_pt)).abs() < 0.1);

        // Recto: x = 200 + spine, width = 200 - spine - fore_edge
        assert!((content.recto.x - (200.0 + spine_pt)).abs() < 0.1);
        assert!((content.recto.width - (200.0 - spine_pt - fore_edge_pt)).abs() < 0.1);

        // Both pages have same height
        let expected_height = 300.0 - top_pt - bottom_pt;
        assert!((content.verso.height - expected_height).abs() < 0.1);
        assert!((content.recto.height - expected_height).abs() < 0.1);
    }

    #[test]
    fn test_spread_content_with_cuts() {
        let spread_pos = SpreadPosition::empty(Point::new(0.0, 0.0), 400.0, 300.0, false, 0);
        let margins = default_margins();
        let cut_edges = SpreadCutEdges {
            top: true,
            bottom: false,
            left: true,
            right: false,
        };

        let content = calculate_spread_content(&spread_pos, &margins, cut_edges);

        // With cut on left, verso uses cut_mm instead of fore_edge_mm
        let cut_pt = mm_to_pt(5.0);
        assert!((content.verso.x - cut_pt).abs() < 0.1);

        // With cut on top, both pages have reduced height
        let top_pt = mm_to_pt(10.0);
        let bottom_pt = mm_to_pt(10.0);
        let expected_height = 300.0 - (top_pt + cut_pt) - bottom_pt;
        assert!((content.verso.height - expected_height).abs() < 0.1);
    }

}
