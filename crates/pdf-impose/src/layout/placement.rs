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
use crate::layout::creep::creep_for_depth_mm;
use crate::layout::slots::slot_content_rect;
use crate::types::{CreepConfig, LeafMargins, ScalingMode};

use super::{Edge, PagePlacement, Rect, SheetSlot, SpreadCutEdges};

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
// Slot-Based Placement
// =============================================================================

/// Place all source pages onto their slots, applying creep compensation.
///
/// Each [`SheetSlot`] carries everything this function needs: physical
/// position, rotation, leaf depth, and which edge is the spine. Creep
/// shifts the content toward the slot's spine edge by an amount derived
/// from `creep` and `slot.leaf_depth`.
///
/// # Arguments
/// * `slots` — one slot per page position on the printed face. Order is
///   `[verso0, recto0, verso1, recto1, …]`, grouped by spread.
/// * `cut_edges` — one entry per *spread* (so `slots.len() / 2` entries),
///   describing which edges of the spread have a cut.
/// * `source_dimensions` — page sizes for the source document.
/// * `leaf_margins` — the four margins around each leaf.
/// * `scaling_mode` — fit/fill/none/stretch.
/// * `creep` — creep configuration; pass [`CreepConfig::None`] for bindings
///   (perfect/spiral/side-stitch) where there is no fold geometry.
pub fn place_slots(
    slots: &[SheetSlot],
    cut_edges: &[SpreadCutEdges],
    source_dimensions: &[(f32, f32)],
    leaf_margins: &LeafMargins,
    scaling_mode: ScalingMode,
    creep: CreepConfig,
) -> Vec<PagePlacement> {
    let mut placements = Vec::with_capacity(slots.len());
    for (slot_idx, slot) in slots.iter().enumerate() {
        let Some(source_idx) = slot.source_page else {
            continue;
        };
        let cuts = cut_edges
            .get(slot_idx / 2)
            .copied()
            .unwrap_or_else(SpreadCutEdges::none);

        let content_area = slot_content_rect(slot, leaf_margins, cuts);
        let (src_w, src_h) = source_dimensions
            .get(source_idx)
            .copied()
            .unwrap_or(DEFAULT_PAGE_DIMENSIONS);

        let scale = calculate_scale(
            src_w,
            src_h,
            content_area.width,
            content_area.height,
            scaling_mode,
        );
        let scaled_w = src_w * scale;
        let scaled_h = src_h * scale;

        // Align toward the spine edge of this slot.
        let (mut x, mut y) = align_toward_spine(&content_area, scaled_w, scaled_h, slot.spine_edge);

        // Apply creep: shift content toward the spine by an amount
        // proportional to leaf depth.
        let shift_pt = crate::constants::mm_to_pt(creep_for_depth_mm(creep, slot.leaf_depth));
        match slot.spine_edge {
            Edge::Right => x += shift_pt,
            Edge::Left => x -= shift_pt,
            Edge::Top => y += shift_pt,
            Edge::Bottom => y -= shift_pt,
        }

        placements.push(PagePlacement {
            source_page: Some(source_idx),
            content_rect: Rect::new(x, y, scaled_w, scaled_h),
            rotation_degrees: slot.rotation_degrees(),
            scale,
        });
    }
    placements
}

/// Align scaled content toward the slot's spine edge, centered on the orthogonal axis.
fn align_toward_spine(
    content_area: &Rect,
    scaled_w: f32,
    scaled_h: f32,
    spine: Edge,
) -> (f32, f32) {
    match spine {
        Edge::Right => {
            // Push to the right: content's right edge meets content_area's right.
            let x = content_area.right() - scaled_w;
            let y = content_area.y + (content_area.height - scaled_h) / 2.0;
            (x, y)
        }
        Edge::Left => {
            let x = content_area.x;
            let y = content_area.y + (content_area.height - scaled_h) / 2.0;
            (x, y)
        }
        Edge::Top => {
            let x = content_area.x + (content_area.width - scaled_w) / 2.0;
            let y = content_area.top() - scaled_h;
            (x, y)
        }
        Edge::Bottom => {
            let x = content_area.x + (content_area.width - scaled_w) / 2.0;
            let y = content_area.y;
            (x, y)
        }
    }
}

#[cfg(test)]
#[path = "tests/placement_tests.rs"]
mod tests;
