//! `SheetSlot` generation: the bridge between fold geometry and page rendering.
//!
//! Given an arrangement, a sheet's position in the signature, and which face
//! is being printed, [`build_sheet_slots`] returns one [`SheetSlot`] per page
//! position on that face — each carrying its source page (or blank), rotation,
//! pre-computed leaf depth, and spine-edge orientation.
//!
//! This module is purely a *transform* over already-existing data:
//! - Sheet partitioning comes from [`super::arrangement::calculate_spread_positions`].
//! - Page-order tables come from [`super::page_order::page_order_for_arrangement`].
//! - Multi-sheet nesting comes from [`super::page_order::build_nesting_remap`].
//!
//! The hand-derived page-order tables remain the source of truth for which
//! signature page lands at each slot. Slots never re-derive that mapping —
//! they consume it and bake the resulting per-slot leaf depth in for downstream
//! use.
//!
//! ## Why per-slot
//!
//! The two pages of a printed spread end up on *different leaves at different
//! depths* after folding (see `memory/imposition_layout_rules.md` and the
//! [`creep`](super::creep) module docs). Modeling slots independently is what
//! makes correct per-page creep, per-leaf marks, and any future per-leaf
//! feature straightforward — the caller can act on each page's geometry
//! without reconstructing the verso/recto pairing.

use crate::constants::mm_to_pt;
use crate::layout::arrangement::calculate_spread_positions;
use crate::layout::page_order::{build_nesting_remap, page_order_for_arrangement};
use crate::types::{LeafMargins, PageArrangement};

use super::{Edge, Rect, SheetSide, SheetSlot, SpreadCutEdges};

/// Where a sheet sits in its signature.
#[derive(Debug, Clone, Copy)]
pub struct SheetPosition {
    /// 0-based index of this sheet within its signature (0 = outermost).
    pub sheet_idx: usize,
    /// Total sheets nested in this signature.
    pub sheets_per_signature: usize,
    /// 0-based source-page index of the first page in this signature.
    pub sig_start: usize,
}

/// Build all slots for one printed face of one sheet in a signature.
///
/// # Arguments
/// * `arrangement` — folio/quarto/octavo (controls sheet partitioning and page order).
/// * `leaf_bounds` — printable area inside sheet margins.
/// * `leaf_margins` — needed to compute the cut-gap for sheet partitioning.
/// * `position` — this sheet's index in its signature plus the signature's
///   first source-page index.
/// * `total_source_pages` — total source pages available; slots beyond this
///   are returned with `source_page = None`.
/// * `side` — which face of the sheet (front/back).
///
/// # Returns
/// One slot per page position on the face, in `[verso0, recto0, verso1,
/// recto1, …]` order — i.e. grouped by spread, with verso first within each
/// pair. This ordering is preserved from the page-order tables and matters
/// because parallel arrays of `SpreadCutEdges` (one per spread) are looked up
/// as `cut_edges[slot_idx / 2]`.
pub fn build_sheet_slots(
    arrangement: PageArrangement,
    leaf_bounds: Rect,
    leaf_margins: &LeafMargins,
    position: SheetPosition,
    total_source_pages: usize,
    side: SheetSide,
) -> Vec<SheetSlot> {
    let spreads = calculate_spread_positions(arrangement, leaf_bounds, leaf_margins);
    let (front_order, back_order) = page_order_for_arrangement(arrangement);
    let order = match side {
        SheetSide::Front => &front_order,
        SheetSide::Back => &back_order,
    };
    let pages_per_sheet = arrangement.pages_per_sheet();
    let remap = build_nesting_remap(
        position.sheet_idx,
        position.sheets_per_signature,
        pages_per_sheet,
    );

    let total_page_cols = arrangement.grid_dimensions().0;
    let spreads_per_row = total_page_cols / 2;

    let mut slots = Vec::with_capacity(order.len());
    for (spread_idx, pair) in order.chunks(2).enumerate() {
        let spread_pos = &spreads[spread_idx];
        let bounds = spread_pos.bounds();
        let half_width = bounds.width / 2.0;

        let verso_rect = Rect::new(bounds.x, bounds.y, half_width, bounds.height);
        let recto_rect = Rect::new(bounds.x + half_width, bounds.y, half_width, bounds.height);

        let verso_sig_idx = remap[pair[0]];
        let recto_sig_idx = remap[pair[1]];

        // Page-col indices of this spread's two slots on the sheet.
        let spread_col_in_row = spread_idx % spreads_per_row;
        let verso_col = 2 * spread_col_in_row;
        let recto_col = verso_col + 1;

        let total_leaves = position.sheets_per_signature * pages_per_sheet / 2;

        slots.push(SheetSlot {
            rect: verso_rect,
            rotated: spread_pos.rotated,
            leaf_depth: leaf_creep_depth(verso_sig_idx, total_leaves),
            spine_edge: spine_edge_for_col(verso_col, total_page_cols),
            source_page: filter_in_range(position.sig_start + verso_sig_idx, total_source_pages),
        });

        slots.push(SheetSlot {
            rect: recto_rect,
            rotated: spread_pos.rotated,
            leaf_depth: leaf_creep_depth(recto_sig_idx, total_leaves),
            spine_edge: spine_edge_for_col(recto_col, total_page_cols),
            source_page: filter_in_range(position.sig_start + recto_sig_idx, total_source_pages),
        });
    }

    slots
}

fn filter_in_range(idx: usize, total: usize) -> Option<usize> {
    if idx < total { Some(idx) } else { None }
}

/// Fore-edge creep depth of the leaf containing `sig_page_idx`.
///
/// Creep at the fore-edge is driven by how many paper layers wrap around this
/// leaf at the spine. In a folded N-sheet signature with `total_leaves` leaves,
/// the bound book's leaves run `0..total_leaves` from front cover to back cover —
/// but a single physical sheet contributes *two* leaves to the bundle, one near
/// the front (leaf k) and one near the back (leaf `total_leaves - 1 - k`),
/// connected by the sheet's spine fold. Both ends of the same sheet sit at the
/// same nesting depth in the spine stack: the outermost sheet wraps around the
/// whole bundle, so its two leaves (front and back covers) are both at depth 0.
///
/// Hence the depth of leaf `k` is its distance from whichever cover it's
/// closer to: `min(k, total_leaves - 1 - k)`.
fn leaf_creep_depth(sig_page_idx: usize, total_leaves: usize) -> usize {
    let leaf_idx = sig_page_idx / 2;
    leaf_idx.min(total_leaves.saturating_sub(1).saturating_sub(leaf_idx))
}

/// Determine the spine-fold edge of a cell at page-col `col` within an
/// arrangement that has `total_cols` page-cols across the sheet.
///
/// The spine fold sits at the central col boundary (between cols `half - 1`
/// and `half` where `half = total_cols / 2`). Cells directly adjacent to that
/// fold get their spine on the side that faces the fold. Cells further away
/// have additional vertical folds (the tail-cut fold in octavo) between them
/// and the spine; each such fold inverts the direction, because the fold
/// physically wraps that cell around to the opposite side of the bound stack.
///
/// Examples:
/// - Folio / quarto (`total_cols = 2`): each cell is adjacent to the spine.
///   Col 0 → `Right`, col 1 → `Left`.
/// - Octavo (`total_cols = 4`): cols 1 and 2 are adjacent (`spine_edge`
///   points inward); cols 0 and 3 are separated by one tail-cut fold each, so
///   their `spine_edge` is inverted (col 0 → `Left`, col 3 → `Right`).
fn spine_edge_for_col(col: usize, total_cols: usize) -> Edge {
    let half = total_cols / 2;
    let (toward, dist) = if col < half {
        (Edge::Right, half - 1 - col)
    } else {
        (Edge::Left, col - half)
    };
    if dist.is_multiple_of(2) {
        toward
    } else {
        toward.opposite()
    }
}

/// Compute the content rect inside a slot, given leaf margins and the cut
/// edges of the slot's parent spread.
///
/// This is the per-slot replacement for the old paired
/// `calculate_spread_content`. The slot's `spine_edge` tells us which side is
/// the spine (gutter margin) vs the fore-edge; the head and tail margins are
/// always top/bottom. Cut-margin compensation matches the previous
/// behavior:
///
/// 1. Edges of the spread that have a cut (per `cut_edges`) get the trim
///    allowance added on top of the margin — *except* the spine edge, which
///    is a fold and never a cut.
/// 2. If the spread has any vertical cut (left or right), the slot's
///    fore-edge gets the trim allowance added too. This keeps verso and
///    recto pages on adjacent slots the same width — the same compensation
///    `calculate_spread_content` did via its `has_vertical_cut` flag.
pub fn slot_content_rect(
    slot: &SheetSlot,
    margins: &LeafMargins,
    cut_edges: SpreadCutEdges,
) -> Rect {
    let spine_pt = mm_to_pt(margins.spine_mm);
    let fore_edge_pt = mm_to_pt(margins.fore_edge_mm);
    let top_pt = mm_to_pt(margins.top_mm);
    let bottom_pt = mm_to_pt(margins.bottom_mm);
    let cut_pt = mm_to_pt(margins.trim_allowance_mm);

    let has_vertical_cut = cut_edges.left || cut_edges.right;
    let fore_edge_total = fore_edge_pt + if has_vertical_cut { cut_pt } else { 0.0 };

    // Default head/tail margins; cuts on the head or tail edge add trim
    // allowance for that edge.
    let mut top_margin = top_pt;
    let mut bottom_margin = bottom_pt;
    if cut_edges.top {
        top_margin += cut_pt;
    }
    if cut_edges.bottom {
        bottom_margin += cut_pt;
    }

    // Resolve left/right insets from the spine_edge.
    let (left_inset, right_inset) = match slot.spine_edge {
        Edge::Right => (fore_edge_total, spine_pt),
        Edge::Left => (spine_pt, fore_edge_total),
        // Top/Bottom spine: not exercised by current arrangements, but let
        // the rect math degrade gracefully (treat both vertical edges as
        // fore-edge-with-cut).
        Edge::Top | Edge::Bottom => (fore_edge_total, fore_edge_total),
    };

    // For a Top/Bottom spine, the spine margin moves into the head/tail
    // direction. (No current arrangement uses this; future fold geometries
    // would.)
    if slot.spine_edge == Edge::Top {
        top_margin = spine_pt + if cut_edges.top { cut_pt } else { 0.0 };
    } else if slot.spine_edge == Edge::Bottom {
        bottom_margin = spine_pt + if cut_edges.bottom { cut_pt } else { 0.0 };
    }

    Rect::new(
        slot.rect.x + left_inset,
        slot.rect.y + bottom_margin,
        slot.rect.width - left_inset - right_inset,
        slot.rect.height - top_margin - bottom_margin,
    )
}

#[cfg(test)]
#[path = "tests/slots_tests.rs"]
mod tests;
