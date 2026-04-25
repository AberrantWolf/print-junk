//! Creep (shingling) compensation
//!
//! When multiple sheets are nested in a folded signature, inner leaves protrude
//! at the fore edge due to accumulated paper thickness. After trimming, inner
//! leaves end up narrower. Creep compensation shifts each page's content toward
//! the spine proportionally to the nesting depth of the leaf it will end up on,
//! so margins stay visually consistent after trim.
//!
//! ## Depth model
//!
//! Depth comes from the page's position within the signature: leaf `k` (0 =
//! outermost) contains pages `2k+1` and `2k+2`, so
//! `depth = signature_page_index / 2` (with `signature_page_index` 0-based).
//! This is the *leaf-pair rule* — see
//! `memory/imposition_layout_rules.md`.
//!
//! The two pages of a printed spread end up on *different* leaves at *different*
//! depths after folding, so creep offsets are per-side (verso, recto) — not
//! per-spread. The page-order tables in [`page_order`] tell us which signature
//! page lands at each slot, so depth per slot falls out of them directly.
//!
//! [`page_order`]: super::page_order

use crate::constants::mm_to_pt;
use crate::layout::SheetSide;
use crate::layout::page_order::{build_nesting_remap, page_order_for_arrangement};
use crate::types::{CreepConfig, PageArrangement};

/// Per-side creep offsets for one printed face: `(verso_pt, recto_pt)` per spread.
///
/// Both values are non-negative and represent a shift *toward the spine*: the
/// verso side shifts right by `verso_pt`, the recto side shifts left by
/// `recto_pt`.
pub type SpreadCreepOffsets = Vec<(f32, f32)>;

/// Creep shift in mm for a leaf at the given depth.
fn creep_for_depth_mm(creep: CreepConfig, depth: usize) -> f32 {
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

/// Per-side creep offsets in points for one face (front or back) of one sheet.
///
/// Returns one `(verso_pt, recto_pt)` tuple per spread on the face, ready to
/// pass to [`super::calculate_spread_placements`]. Returns an empty vec when
/// creep is disabled — callers treat that as "no shift for any spread".
///
/// ## Why per-face
///
/// The left-right flip on the back side means the verso of a back spread lands
/// on a different physical leaf than the verso of the corresponding front
/// spread. Each face must be computed independently.
pub fn creep_offsets_for_face(
    creep: CreepConfig,
    arrangement: PageArrangement,
    sheet_idx: usize,
    sheets_per_signature: usize,
    side: SheetSide,
) -> SpreadCreepOffsets {
    if !creep.is_enabled() {
        return Vec::new();
    }

    let pages_per_sheet = arrangement.pages_per_sheet();
    let (front, back) = page_order_for_arrangement(arrangement);
    let order = match side {
        SheetSide::Front => &front,
        SheetSide::Back => &back,
    };
    let remap = build_nesting_remap(sheet_idx, sheets_per_signature, pages_per_sheet);

    order
        .chunks(2)
        .map(|pair| {
            let verso_depth = remap[pair[0]] / 2;
            let recto_depth = remap[pair[1]] / 2;
            (
                mm_to_pt(creep_for_depth_mm(creep, verso_depth)),
                mm_to_pt(creep_for_depth_mm(creep, recto_depth)),
            )
        })
        .collect()
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
