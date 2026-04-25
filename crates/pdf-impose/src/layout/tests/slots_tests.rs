//! Tests for sheet-slot generation and per-slot content-area math.
//!
//! Correctness tests pin slot output against the page-order tables — the
//! same source of truth the creep tests use.

use super::*;
use crate::layout::arrangement::calculate_cut_edges;
use crate::layout::slots::SheetPosition;
use crate::types::{LeafMargins, PageArrangement};

fn margins_zero_with_5mm_spine() -> LeafMargins {
    LeafMargins {
        top_mm: 0.0,
        bottom_mm: 0.0,
        fore_edge_mm: 0.0,
        spine_mm: 5.0,
        trim_allowance_mm: 3.0,
    }
}

fn unit_leaf_bounds() -> Rect {
    Rect::new(0.0, 0.0, 800.0, 600.0)
}

fn pos(sheet_idx: usize, sheets_per_signature: usize, sig_start: usize) -> SheetPosition {
    SheetPosition {
        sheet_idx,
        sheets_per_signature,
        sig_start,
    }
}

// =============================================================================
// build_sheet_slots: page assignments and depths
// =============================================================================

#[test]
fn folio_single_sheet_front_slots() {
    // Front order [3, 0]: verso=page 4 (leaf 1), recto=page 1 (leaf 0).
    // Single-sheet folio has 2 leaves total; both are "covers" at depth 0.
    let slots = build_sheet_slots(
        PageArrangement::Folio,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(0, 1, 0),
        4,
        SheetSide::Front,
    );

    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].source_page, Some(3));
    assert_eq!(slots[0].leaf_depth, 0);
    assert_eq!(slots[0].spine_edge, Edge::Right);
    assert!(!slots[0].rotated);

    assert_eq!(slots[1].source_page, Some(0));
    assert_eq!(slots[1].leaf_depth, 0);
    assert_eq!(slots[1].spine_edge, Edge::Left);
}

#[test]
fn quarto_single_sheet_front_depth_table() {
    let slots = build_sheet_slots(
        PageArrangement::Quarto,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(0, 1, 0),
        8,
        SheetSide::Front,
    );

    let depths: Vec<usize> = slots.iter().map(|s| s.leaf_depth).collect();
    // Single-sheet quarto: 4 leaves, depths min(k, 3-k).
    // Bottom spread (pages 8=leaf 3, 1=leaf 0): depths 0, 0 (both covers).
    // Top spread (pages 5=leaf 2, 4=leaf 1): depths 1, 1 (innermost pair).
    assert_eq!(depths, vec![0, 0, 1, 1]);
}

#[test]
fn folio_spine_edges_point_at_each_other() {
    let slots = build_sheet_slots(
        PageArrangement::Folio,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(0, 1, 0),
        4,
        SheetSide::Front,
    );
    // The single fold is the spine; both cells border it.
    assert_eq!(slots[0].spine_edge, Edge::Right);
    assert_eq!(slots[1].spine_edge, Edge::Left);
}

#[test]
fn quarto_spine_edges_point_at_each_other() {
    let slots = build_sheet_slots(
        PageArrangement::Quarto,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(0, 1, 0),
        8,
        SheetSide::Front,
    );
    // Quarto: 1 vertical fold (the spine). Same cell-to-spine relationship
    // as folio, applied to each row.
    let edges: Vec<Edge> = slots.iter().map(|s| s.spine_edge).collect();
    assert_eq!(
        edges,
        vec![Edge::Right, Edge::Left, Edge::Right, Edge::Left]
    );
}

#[test]
fn octavo_spine_edges_invert_outside_the_tail_cut() {
    // Octavo has 2 vertical folds: the central spine fold (between cols 1
    // and 2) and the tail-cut fold at cols 0/1 and 2/3. Cells separated from
    // the spine by the tail-cut fold (cols 0 and 3) get their spine_edge
    // inverted. Pattern across the 4 page-cols:
    //   col 0 → Left  (sheet-left wraps via tail-cut to spine)
    //   col 1 → Right (cell's right is the spine fold)
    //   col 2 → Left  (cell's left is the spine fold)
    //   col 3 → Right (sheet-right wraps via tail-cut to spine)
    let slots = build_sheet_slots(
        PageArrangement::Octavo,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(0, 1, 0),
        16,
        SheetSide::Front,
    );
    let edges: Vec<Edge> = slots.iter().map(|s| s.spine_edge).collect();
    // Spread order is BL, BR, TL, TR — so slots are
    // [BL_v, BL_r, BR_v, BR_r, TL_v, TL_r, TR_v, TR_r]
    // Mapped to page-cols: [0, 1, 2, 3, 0, 1, 2, 3].
    assert_eq!(
        edges,
        vec![
            Edge::Left,  // col 0 (BL verso)
            Edge::Right, // col 1 (BL recto)
            Edge::Left,  // col 2 (BR verso)
            Edge::Right, // col 3 (BR recto)
            Edge::Left,  // col 0 (TL verso, top row, rotated — but spine is press-sheet geometry)
            Edge::Right, // col 1 (TL recto)
            Edge::Left,  // col 2 (TR verso)
            Edge::Right, // col 3 (TR recto)
        ]
    );
}

#[test]
fn octavo_single_sheet_front_depth_table() {
    let slots = build_sheet_slots(
        PageArrangement::Octavo,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(0, 1, 0),
        16,
        SheetSide::Front,
    );

    // Single-sheet octavo: 8 leaves, depths min(k, 7-k):
    //   leaf 0..7 → 0, 1, 2, 3, 3, 2, 1, 0
    // BL [v=4=leaf 1, r=13=leaf 6] → 1, 1
    // BR [v=16=leaf 7, r=1=leaf 0] → 0, 0
    // TL [v=5=leaf 2, r=12=leaf 5] → 2, 2
    // TR [v=9=leaf 4, r=8=leaf 3]  → 3, 3
    let depths: Vec<usize> = slots.iter().map(|s| s.leaf_depth).collect();
    assert_eq!(depths, vec![1, 1, 0, 0, 2, 2, 3, 3]);
}

#[test]
fn octavo_top_row_slots_inherit_rotation() {
    let slots = build_sheet_slots(
        PageArrangement::Octavo,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(0, 1, 0),
        16,
        SheetSide::Front,
    );

    // Bottom row (slots 0..4) not rotated; top row (slots 4..8) rotated.
    for (i, slot) in slots.iter().enumerate() {
        let expected = i >= 4;
        assert_eq!(
            slot.rotated, expected,
            "slot {i} rotation mismatch: got {}",
            slot.rotated
        );
    }
}

#[test]
fn back_face_uses_back_order() {
    // Back order for folio is [1, 2]: verso=page 2 (leaf 0), recto=page 3
    // (leaf 1). This is the *reverse* of front, proving we picked the back
    // order rather than reusing front. Single-sheet folio has 2 leaves total,
    // both at depth 0.
    let slots = build_sheet_slots(
        PageArrangement::Folio,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(0, 1, 0),
        4,
        SheetSide::Back,
    );

    assert_eq!(slots[0].source_page, Some(1));
    assert_eq!(slots[0].leaf_depth, 0);
    assert_eq!(slots[1].source_page, Some(2));
    assert_eq!(slots[1].leaf_depth, 0);
}

#[test]
fn slots_blank_when_past_total_pages() {
    // Total source pages = 2, but folio needs 4 page slots.
    let slots = build_sheet_slots(
        PageArrangement::Folio,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(0, 1, 0),
        2,
        SheetSide::Front,
    );

    // Front order [3, 0]. Verso wants page 3 (out of range), recto wants 0.
    assert_eq!(slots[0].source_page, None, "verso past EOF should be blank");
    assert_eq!(slots[1].source_page, Some(0));
}

#[test]
fn two_sheet_folio_outer_sheet_both_leaves_at_depth_zero() {
    // 2-sheet folio (4 leaves): sheet 0 carries leaf 0 (front cover, page 1)
    // and leaf 3 (back cover, page 8). Both halves of the outermost physical
    // sheet wrap the bundle together, so both are at depth 0.
    let outer = build_sheet_slots(
        PageArrangement::Folio,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(0, 2, 0),
        8,
        SheetSide::Front,
    );
    assert_eq!(
        outer[0].leaf_depth, 0,
        "verso of outer sheet (page 8 = leaf 3, back cover)"
    );
    assert_eq!(
        outer[1].leaf_depth, 0,
        "recto of outer sheet (page 1 = leaf 0, front cover)"
    );

    // Inner sheet carries the two centermost leaves (1, 2) — both at depth 1.
    let inner = build_sheet_slots(
        PageArrangement::Folio,
        unit_leaf_bounds(),
        &margins_zero_with_5mm_spine(),
        pos(1, 2, 0),
        8,
        SheetSide::Front,
    );
    assert_eq!(inner[0].leaf_depth, 1, "verso of inner sheet");
    assert_eq!(inner[1].leaf_depth, 1, "recto of inner sheet");
}

#[test]
fn four_sheet_folio_depth_table() {
    // 4-sheet folio (8 leaves): each physical sheet contributes a leaf-pair
    // {k, 7-k}, both at depth k. This is the user-reported case.
    let expected: Vec<(usize, [usize; 2])> = vec![
        (0, [0, 0]), // pages 16, 1: leaves 7, 0
        (1, [1, 1]), // pages 14, 3: leaves 6, 1
        (2, [2, 2]), // pages 12, 5: leaves 5, 2
        (3, [3, 3]), // pages 10, 7: leaves 4, 3
    ];
    for (sheet_idx, want) in expected {
        let slots = build_sheet_slots(
            PageArrangement::Folio,
            unit_leaf_bounds(),
            &margins_zero_with_5mm_spine(),
            pos(sheet_idx, 4, 0),
            16,
            SheetSide::Front,
        );
        assert_eq!(
            [slots[0].leaf_depth, slots[1].leaf_depth],
            want,
            "sheet {sheet_idx}: both slots should share creep depth"
        );
    }
}

// =============================================================================
// slot_content_rect: per-slot margin math
// =============================================================================

#[test]
fn content_rect_spine_right_vs_spine_left_are_mirrored() {
    let margins = LeafMargins {
        top_mm: 0.0,
        bottom_mm: 0.0,
        fore_edge_mm: 4.0,
        spine_mm: 10.0,
        trim_allowance_mm: 3.0,
    };

    let rect = Rect::new(100.0, 200.0, 200.0, 300.0);
    let no_cuts = SpreadCutEdges::none();

    let verso_slot = SheetSlot {
        rect,
        rotated: false,
        leaf_depth: 0,
        spine_edge: Edge::Right,
        source_page: Some(0),
    };
    let recto_slot = SheetSlot {
        rect,
        rotated: false,
        leaf_depth: 0,
        spine_edge: Edge::Left,
        source_page: Some(0),
    };

    let verso_content = slot_content_rect(&verso_slot, &margins, no_cuts);
    let recto_content = slot_content_rect(&recto_slot, &margins, no_cuts);

    // Same width, same height; left/right insets mirrored.
    assert!((verso_content.width - recto_content.width).abs() < 0.001);
    assert!((verso_content.height - recto_content.height).abs() < 0.001);

    let verso_left_inset = verso_content.x - rect.x;
    let recto_left_inset = recto_content.x - rect.x;
    let verso_right_inset = (rect.x + rect.width) - (verso_content.x + verso_content.width);
    let recto_right_inset = (rect.x + rect.width) - (recto_content.x + recto_content.width);

    // Verso: fore on left, spine on right. Recto: spine on left, fore on right.
    assert!(
        (verso_left_inset - recto_right_inset).abs() < 0.001,
        "verso left = recto right (both fore-edge)"
    );
    assert!(
        (verso_right_inset - recto_left_inset).abs() < 0.001,
        "verso right = recto left (both spine)"
    );
}

#[test]
fn content_rect_inflates_fore_edge_when_spread_has_vertical_cut() {
    let margins = LeafMargins {
        top_mm: 0.0,
        bottom_mm: 0.0,
        fore_edge_mm: 4.0,
        spine_mm: 10.0,
        trim_allowance_mm: 3.0,
    };
    let rect = Rect::new(0.0, 0.0, 200.0, 300.0);

    let slot = SheetSlot {
        rect,
        rotated: false,
        leaf_depth: 0,
        spine_edge: Edge::Right,
        source_page: Some(0),
    };

    let no_cuts = slot_content_rect(&slot, &margins, SpreadCutEdges::none());
    let with_left_cut = slot_content_rect(
        &slot,
        &margins,
        SpreadCutEdges {
            left: true,
            ..SpreadCutEdges::none()
        },
    );

    // The cut is on the spread's left, but our spine_edge is Right (so our
    // fore-edge is on the left). The fore-edge inset should grow by the trim
    // allowance — same compensation that kept octavo verso/recto widths
    // matched in the old paired calculation.
    let cut_pt = crate::constants::mm_to_pt(margins.trim_allowance_mm);
    assert!(
        (no_cuts.width - with_left_cut.width - cut_pt).abs() < 0.001,
        "fore-edge cut should narrow content by trim allowance"
    );
}

#[test]
fn content_rect_adds_cut_margin_to_head_when_top_edge_is_cut() {
    let margins = LeafMargins {
        top_mm: 5.0,
        bottom_mm: 5.0,
        fore_edge_mm: 0.0,
        spine_mm: 0.0,
        trim_allowance_mm: 3.0,
    };
    let rect = Rect::new(0.0, 0.0, 200.0, 300.0);

    let slot = SheetSlot {
        rect,
        rotated: false,
        leaf_depth: 0,
        spine_edge: Edge::Right,
        source_page: Some(0),
    };

    let no_cuts = slot_content_rect(&slot, &margins, SpreadCutEdges::none());
    let top_cut = slot_content_rect(
        &slot,
        &margins,
        SpreadCutEdges {
            top: true,
            ..SpreadCutEdges::none()
        },
    );

    let cut_pt = crate::constants::mm_to_pt(margins.trim_allowance_mm);
    assert!(
        (no_cuts.height - top_cut.height - cut_pt).abs() < 0.001,
        "head cut should reduce content height by trim allowance"
    );
}

// =============================================================================
// Cross-check: octavo same-width property still holds via the new helper
// =============================================================================

#[test]
fn octavo_all_slots_have_uniform_content_width() {
    let margins = LeafMargins {
        top_mm: 5.0,
        bottom_mm: 5.0,
        fore_edge_mm: 4.0,
        spine_mm: 10.0,
        trim_allowance_mm: 3.0,
    };
    let leaf_bounds = unit_leaf_bounds();

    let slots = build_sheet_slots(
        PageArrangement::Octavo,
        leaf_bounds,
        &margins,
        pos(0, 1, 0),
        16,
        SheetSide::Front,
    );
    let cut_edges = calculate_cut_edges(PageArrangement::Octavo);

    let widths: Vec<f32> = slots
        .iter()
        .enumerate()
        .map(|(i, slot)| slot_content_rect(slot, &margins, cut_edges[i / 2]).width)
        .collect();

    let first = widths[0];
    for (i, w) in widths.iter().enumerate() {
        assert!(
            (w - first).abs() < 0.001,
            "slot {i} width {w} differs from first {first}"
        );
    }
}
