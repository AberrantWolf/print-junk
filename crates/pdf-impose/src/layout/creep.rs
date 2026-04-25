//! Creep (shingling) compensation — math primitives.
//!
//! When multiple sheets are nested in a folded signature, inner leaves protrude
//! at the fore edge due to accumulated paper thickness. After trimming, inner
//! leaves end up narrower. Creep compensation shifts each page's content
//! toward the spine proportionally to the nesting depth of the leaf it will
//! end up on, so margins stay visually consistent after trim.
//!
//! ## Depth model
//!
//! Depth comes from the page's position within the signature: leaf `k` (0 =
//! outermost) contains pages `2k+1` and `2k+2`, so
//! `depth = signature_page_index / 2` (with `signature_page_index` 0-based).
//! This is the *leaf-pair rule* — see `memory/imposition_layout_rules.md`.
//!
//! The two pages of a printed spread end up on *different* leaves at *different*
//! depths after folding, so creep is applied *per-slot*, not per-spread. The
//! [`super::slots`] module bakes the right depth into each [`super::SheetSlot`];
//! [`super::placement::place_slots`] consumes that depth and shifts the
//! content toward `slot.spine_edge`. This module just provides the math:
//! given a depth, what's the shift?

use crate::types::{CreepConfig, PageArrangement};

/// Creep shift in mm for a leaf at the given depth.
///
/// Pure math primitive. The dispatch (which slot has which depth, which face,
/// which sheet) lives in [`super::slots`] and the actual shift is applied by
/// [`super::placement::place_slots`].
pub(crate) fn creep_for_depth_mm(creep: CreepConfig, depth: usize) -> f32 {
    match creep {
        CreepConfig::None => 0.0,
        CreepConfig::PerLayer { creep_per_layer_mm } => depth as f32 * creep_per_layer_mm,
        CreepConfig::FromCaliper { paper_thickness_mm } => {
            // Fold geometry: each paper layer wraps around the spine fold, and
            // the arc difference between outer and inner surfaces is π·t per
            // fold. Half of that appears as fore-edge displacement per layer.
            depth as f32 * std::f32::consts::PI * paper_thickness_mm / 2.0
        }
    }
}

/// Maximum creep offset in mm across all leaves of a signature.
///
/// This is the shift for the innermost leaf, useful for spine-margin warnings
/// and guiding the user to widen their gutter.
pub fn max_creep_offset_mm(
    creep: CreepConfig,
    arrangement: PageArrangement,
    sheets_per_signature: usize,
) -> f32 {
    if sheets_per_signature == 0 {
        return 0.0;
    }
    let total_leaves = sheets_per_signature * arrangement.pages_per_sheet() / 2;
    creep_for_depth_mm(creep, total_leaves - 1)
}

#[cfg(test)]
#[path = "tests/creep_tests.rs"]
mod tests;
