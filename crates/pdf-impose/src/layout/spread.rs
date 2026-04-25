//! Sheet partitioning into spread regions.
//!
//! A spread is a piece of fold geometry: a region of the press sheet that
//! becomes two facing pages after folding. This module places spread regions
//! on a press sheet for each arrangement; per-page content rects (margins,
//! cuts) are computed in [`super::slots`].
//!
//! - **Folio**: 1 spread (no inner cuts)
//! - **Quarto**: 2 spreads stacked vertically (top rotated 180°, head cut between)
//! - **Octavo**: 4 spreads in 2×2 grid (top row rotated 180°, head + center cuts)

use super::{Point, Rect, SpreadPosition};

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
/// Returns [`bottom_spread`, `top_spread`] where top is rotated 180 degrees.
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

#[cfg(test)]
#[path = "tests/spread_tests.rs"]
mod tests;
