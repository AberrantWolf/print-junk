//! Arrangement layouts as compositions of spreads
//!
//! This module implements the hierarchical layout model where each
//! arrangement is built from simpler ones:
//!
//! - **Folio** = 1 spread (the base case)
//! - **Quarto** = 2 folios stacked vertically (top rotated 180 degrees)
//! - **Octavo** = 2 quartos side-by-side
//!
//! This compositional approach means:
//! 1. The same layout logic works for portrait and landscape orientations
//! 2. Adding new arrangements (e.g., sextodecimo) is straightforward
//! 3. Cut positions are derived from the composition structure

use crate::constants::mm_to_pt;
use crate::types::{LeafMargins, PageArrangement};

use super::spread::{create_folio_spread, create_octavo_spreads, create_quarto_spreads};
use super::{Rect, SpreadCutEdges, SpreadPosition};

// =============================================================================
// Arrangement Configuration
// =============================================================================

/// Configuration for an arrangement layout
#[derive(Debug, Clone)]
pub struct ArrangementConfig {
    /// Number of spread columns
    pub cols: usize,
    /// Number of spread rows
    pub rows: usize,
    /// Total number of spreads
    pub spread_count: usize,
    /// Number of pages per signature
    pub pages_per_signature: usize,
}

impl ArrangementConfig {
    /// Get configuration for a page arrangement
    pub fn for_arrangement(arrangement: PageArrangement) -> Self {
        match arrangement {
            PageArrangement::Folio => Self {
                cols: 1,
                rows: 1,
                spread_count: 1,
                pages_per_signature: 4,
            },
            PageArrangement::Quarto => Self {
                cols: 1,
                rows: 2,
                spread_count: 2,
                pages_per_signature: 8,
            },
            PageArrangement::Octavo => Self {
                cols: 2,
                rows: 2,
                spread_count: 4,
                pages_per_signature: 16,
            },
            PageArrangement::Custom {
                pages_per_signature,
            } => {
                // Treat custom as folio-like (single spread per sheet)
                Self {
                    cols: 1,
                    rows: 1,
                    spread_count: 1,
                    pages_per_signature,
                }
            }
        }
    }
}

// =============================================================================
// Main Layout Function
// =============================================================================

/// Calculate spread positions for a given arrangement.
///
/// This is the main entry point for the spread-based layout system.
/// It returns spread positions WITHOUT page assignments - those are
/// added separately by the page_order module.
///
/// # Arguments
/// * `arrangement` - The page arrangement (folio, quarto, octavo, custom)
/// * `leaf_bounds` - The printable area (inside sheet margins)
/// * `leaf_margins` - Margin configuration for cut gaps
///
/// # Returns
/// Vector of spread positions in layout order:
/// - Folio: [spread]
/// - Quarto: [bottom, top]
/// - Octavo: [bottom-left, bottom-right, top-left, top-right]
pub fn calculate_spread_positions(
    arrangement: PageArrangement,
    leaf_bounds: Rect,
    leaf_margins: &LeafMargins,
) -> Vec<SpreadPosition> {
    let cut_gap = mm_to_pt(leaf_margins.cut_mm);

    match arrangement {
        PageArrangement::Folio => vec![create_folio_spread(leaf_bounds)],
        PageArrangement::Quarto => create_quarto_spreads(leaf_bounds, cut_gap),
        PageArrangement::Octavo => create_octavo_spreads(leaf_bounds, cut_gap, cut_gap),
        PageArrangement::Custom { .. } => {
            // Custom uses folio layout
            vec![create_folio_spread(leaf_bounds)]
        }
    }
}

/// Get cut edges for each spread position in an arrangement.
///
/// Returns a vector parallel to the spread positions, indicating
/// which edges of each spread have cut lines.
pub fn calculate_cut_edges(arrangement: PageArrangement) -> Vec<SpreadCutEdges> {
    let config = ArrangementConfig::for_arrangement(arrangement);

    (0..config.spread_count)
        .map(|i| spread_cut_edges(i, config.cols, config.rows))
        .collect()
}

/// Determine cut edges for a single spread position.
fn spread_cut_edges(spread_index: usize, cols: usize, rows: usize) -> SpreadCutEdges {
    if cols == 1 && rows == 1 {
        // Folio: no cuts
        return SpreadCutEdges::none();
    }

    let row = spread_index / cols;
    let col = spread_index % cols;

    // Note: row 0 = bottom, row 1 = top (in our layout)
    // A spread has a cut on an edge if there's another spread on that side

    SpreadCutEdges {
        // Cut above if there's a row above (we're not in the top row)
        top: row < rows - 1,
        // Cut below if there's a row below (we're not in the bottom row)
        bottom: row > 0,
        // Cut to left if there's a column to the left
        left: col > 0,
        // Cut to right if there's a column to the right
        right: col < cols - 1,
    }
}

// =============================================================================
// Cut Position Calculation
// =============================================================================

/// Positions of cut lines on a sheet
#[derive(Debug, Clone, Default)]
pub struct CutPositions {
    /// X coordinates of vertical cut lines
    pub vertical: Vec<f32>,
    /// Y coordinates of horizontal cut lines
    pub horizontal: Vec<f32>,
}

impl CutPositions {
    /// Calculate cut positions for an arrangement.
    ///
    /// Cut lines appear between spreads in the arrangement.
    pub fn for_arrangement(
        arrangement: PageArrangement,
        leaf_bounds: &Rect,
        _cut_margin_pt: f32,
    ) -> Self {
        match arrangement {
            PageArrangement::Folio => CutPositions::default(),
            PageArrangement::Quarto => {
                // Horizontal cut in the middle
                let mid_y = leaf_bounds.y + leaf_bounds.height / 2.0;
                CutPositions {
                    vertical: vec![],
                    horizontal: vec![mid_y],
                }
            }
            PageArrangement::Octavo => {
                // Both horizontal and vertical cuts
                let mid_x = leaf_bounds.x + leaf_bounds.width / 2.0;
                let mid_y = leaf_bounds.y + leaf_bounds.height / 2.0;
                CutPositions {
                    vertical: vec![mid_x],
                    horizontal: vec![mid_y],
                }
            }
            PageArrangement::Custom { .. } => CutPositions::default(),
        }
    }

    /// Check if there are any cuts
    pub fn any(&self) -> bool {
        !self.vertical.is_empty() || !self.horizontal.is_empty()
    }
}

#[cfg(test)]
#[path = "tests/arrangement_tests.rs"]
mod tests;
