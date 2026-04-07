//! Page placement within spreads
//!
//! This module calculates where pages are placed within spread content areas,
//! handling scaling and alignment.
//!
//! ## Margin Model
//!
//! ```text
//! +----------------------------------------------------------+
//! |                    Sheet Margin                          |
//! |  +----------------------------------------------------+  |
//! |  |                  Top Leaf Margin                   |  |
//! |  |  +----------------------------------------------+  |  |
//! |  |  |  Fore    |          |          |    Fore    |  |  |
//! |  |  |  Edge    |  Spine   |  Spine   |    Edge    |  |  |
//! |  |  |  Margin  |  Margin  |  Margin  |   Margin   |  |  |
//! |  |  |         [Verso]    ||   [Recto]             |  |  |
//! |  |  +----------------------------------------------+  |  |
//! |  |                 Bottom Leaf Margin                 |  |
//! |  +----------------------------------------------------+  |
//! +----------------------------------------------------------+
//! ```
//!
//! For multi-row layouts (Quarto, Octavo), a Cut Margin separates rows.
//! For Octavo, a Cut Margin also separates the center columns.

use crate::constants::DEFAULT_PAGE_DIMENSIONS;
use crate::types::{LeafMargins, ScalingMode};

use super::spread::calculate_spread_content;
use super::{
    GridPosition, PagePlacement, PageSide, Rect, SheetSide, SignatureSlot, SpreadCutEdges,
    SpreadPosition,
};

// =============================================================================
// Scaling
// =============================================================================

/// Calculate scale factor for fitting source to target dimensions.
fn calculate_scale(
    src_width: f32,
    src_height: f32,
    target_width: f32,
    target_height: f32,
    mode: ScalingMode,
) -> f32 {
    if target_width <= 0.0 || target_height <= 0.0 {
        return 1.0;
    }
    if src_width <= 0.0 || src_height <= 0.0 {
        return 1.0;
    }

    let scale_w = target_width / src_width;
    let scale_h = target_height / src_height;

    match mode {
        ScalingMode::Fit => scale_w.min(scale_h),
        ScalingMode::Fill => scale_w.max(scale_h),
        ScalingMode::None => 1.0,
        ScalingMode::Stretch => scale_w,
    }
}

// =============================================================================
// Spread-Based Placement
// =============================================================================

/// Place a single page within a content area.
fn place_single_page(
    source_idx: usize,
    content_area: &Rect,
    source_width: f32,
    source_height: f32,
    scaling_mode: ScalingMode,
    spread_pos: &SpreadPosition,
    page_side: PageSide,
    sheet_side: SheetSide,
) -> PagePlacement {
    let scale = calculate_scale(
        source_width,
        source_height,
        content_area.width,
        content_area.height,
        scaling_mode,
    );

    let scaled_width = source_width * scale;
    let scaled_height = source_height * scale;

    // Align content toward the spine (center of spread)
    let x = match page_side {
        PageSide::Verso => {
            // Verso: push content right (toward spine)
            content_area.right() - scaled_width
        }
        PageSide::Recto => {
            // Recto: push content left (toward spine)
            content_area.x
        }
    };

    // Center vertically
    let y = content_area.y + (content_area.height - scaled_height) / 2.0;

    // Create a SignatureSlot for compatibility with rendering
    let slot = SignatureSlot {
        slot_index: spread_pos.spread_index * 2 + usize::from(page_side.is_recto()),
        sheet_side,
        grid_pos: GridPosition::new(0, usize::from(page_side.is_recto())),
        rotated: spread_pos.rotated,
        page_side,
    };

    PagePlacement {
        source_page: Some(source_idx),
        content_rect: Rect::new(x, y, scaled_width, scaled_height),
        rotation_degrees: spread_pos.rotation_degrees(),
        scale,
        slot,
    }
}

/// Calculate all page placements for a sheet side using the spread-based system.
///
/// # Arguments
/// * `spreads` - Spread positions with page assignments
/// * `cut_edges` - Cut edge information for each spread
/// * `source_dimensions` - Dimensions of all source pages
/// * `leaf_margins` - Margin configuration
/// * `scaling_mode` - How to scale pages
///
/// # Returns
/// Vector of all `PagePlacements` for rendering
pub fn calculate_spread_placements(
    spreads: &[SpreadPosition],
    cut_edges: &[SpreadCutEdges],
    source_dimensions: &[(f32, f32)],
    leaf_margins: &LeafMargins,
    scaling_mode: ScalingMode,
    sheet_side: SheetSide,
) -> Vec<PagePlacement> {
    spreads
        .iter()
        .zip(cut_edges.iter())
        .flat_map(|(spread_pos, cuts)| {
            let content_areas = calculate_spread_content(spread_pos, leaf_margins, *cuts);
            let mut placements = Vec::with_capacity(2);

            // Place verso (left) page
            if let Some(verso_idx) = spread_pos.spread.verso_page {
                let (src_w, src_h) = source_dimensions
                    .get(verso_idx)
                    .copied()
                    .unwrap_or(DEFAULT_PAGE_DIMENSIONS);

                placements.push(place_single_page(
                    verso_idx,
                    &content_areas.verso,
                    src_w,
                    src_h,
                    scaling_mode,
                    spread_pos,
                    PageSide::Verso,
                    sheet_side,
                ));
            }

            // Place recto (right) page
            if let Some(recto_idx) = spread_pos.spread.recto_page {
                let (src_w, src_h) = source_dimensions
                    .get(recto_idx)
                    .copied()
                    .unwrap_or(DEFAULT_PAGE_DIMENSIONS);

                placements.push(place_single_page(
                    recto_idx,
                    &content_areas.recto,
                    src_w,
                    src_h,
                    scaling_mode,
                    spread_pos,
                    PageSide::Recto,
                    sheet_side,
                ));
            }

            placements
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/placement_tests.rs"]
mod tests;
