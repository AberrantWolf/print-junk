//! Tests for creep compensation.
//!
//! Per-leaf depth tables are pinned in `slots_tests.rs` (since slots are
//! the unit that carries depth). These tests focus on:
//! - the `creep_for_depth_mm` math primitive,
//! - `max_creep_offset_mm`,
//! - `place_slots` integration (shift direction and magnitude).

use super::*;
use crate::constants::mm_to_pt;
use crate::layout::arrangement::calculate_cut_edges;
use crate::layout::placement::place_slots;
use crate::layout::slots::{SheetPosition, build_sheet_slots};
use crate::layout::{Rect, SheetSide};
use crate::types::{CreepConfig, LeafMargins, PageArrangement, ScalingMode};

fn default_margins() -> LeafMargins {
    LeafMargins {
        top_mm: 0.0,
        bottom_mm: 0.0,
        fore_edge_mm: 0.0,
        spine_mm: 5.0,
        trim_allowance_mm: 3.0,
    }
}

// =============================================================================
// max_creep_offset_mm
// =============================================================================

#[test]
fn test_max_creep_innermost_leaf() {
    // Max depth is the centermost leaf-pair: depth = (total_leaves - 1) / 2.
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 0.1,
    };
    for (arr, sheets) in [
        (PageArrangement::Folio, 1),
        (PageArrangement::Folio, 3),
        (PageArrangement::Quarto, 1),
        (PageArrangement::Quarto, 2),
        (PageArrangement::Octavo, 1),
        (PageArrangement::Octavo, 2),
    ] {
        let total_leaves = sheets * arr.pages_per_sheet() / 2;
        let expected = ((total_leaves - 1) / 2) as f32 * 0.1;
        let actual = max_creep_offset_mm(creep, arr, sheets);
        assert!(
            (actual - expected).abs() < f32::EPSILON,
            "arr={arr:?} sheets={sheets}: got {actual}, expected {expected}"
        );
    }
}

#[test]
fn test_max_creep_zero_sheets() {
    let actual = max_creep_offset_mm(
        CreepConfig::PerLayer {
            creep_per_layer_mm: 0.1,
        },
        PageArrangement::Folio,
        0,
    );
    assert!(actual.abs() < f32::EPSILON, "got {actual}");
}

#[test]
fn test_max_creep_none_is_zero() {
    let actual = max_creep_offset_mm(CreepConfig::None, PageArrangement::Quarto, 5);
    assert!(actual.abs() < f32::EPSILON, "got {actual}");
}

#[test]
fn test_max_creep_from_caliper_uses_pi_over_two_factor() {
    // Single-sheet quarto: 4 leaves, max depth = (4-1)/2 = 1. π·t/2 per
    // layer = π·0.1/2 mm. (Single-sheet folio has only 2 leaves, both at
    // depth 0, so it would yield 0 — not useful for this assertion.)
    let creep = CreepConfig::FromCaliper {
        paper_thickness_mm: 0.1,
    };
    let actual = max_creep_offset_mm(creep, PageArrangement::Quarto, 1);
    let expected = std::f32::consts::PI * 0.1 / 2.0;
    assert!(
        (actual - expected).abs() < 1e-6,
        "got {actual}, expected {expected}"
    );
}

// =============================================================================
// place_slots integration: creep shifts content toward the spine
// =============================================================================

#[test]
fn test_creep_shifts_verso_right_recto_left() {
    // Quarto single sheet: verso (page 8, leaf 3 → depth 0) doesn't shift;
    // recto (page 1, leaf 0 → depth 0) doesn't shift; but the *top* spread's
    // verso (page 5, leaf 2 → depth 1) shifts right; recto (page 4, leaf 1 →
    // depth 1) shifts left. Verifies direction: toward `spine_edge`.
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = default_margins();
    let cut_edges = calculate_cut_edges(PageArrangement::Quarto);
    let slots = build_sheet_slots(
        PageArrangement::Quarto,
        leaf_bounds,
        &margins,
        SheetPosition {
            sheet_idx: 0,
            sheets_per_signature: 1,
            sig_start: 0,
        },
        8,
        SheetSide::Front,
    );
    let source_dims = vec![(612.0, 792.0); 8];

    let no_creep = place_slots(
        &slots,
        &cut_edges,
        &source_dims,
        &margins,
        ScalingMode::Fit,
        CreepConfig::None,
    );
    let with_creep = place_slots(
        &slots,
        &cut_edges,
        &source_dims,
        &margins,
        ScalingMode::Fit,
        CreepConfig::PerLayer {
            creep_per_layer_mm: 0.5,
        },
    );

    // Bottom spread (slots 0, 1): both leaves are covers, depth 0 → no shift.
    let bot_v_delta = with_creep[0].content_rect.x - no_creep[0].content_rect.x;
    let bot_r_delta = with_creep[1].content_rect.x - no_creep[1].content_rect.x;
    assert!(
        bot_v_delta.abs() < 0.001,
        "bottom verso depth 0: {bot_v_delta}"
    );
    assert!(
        bot_r_delta.abs() < 0.001,
        "bottom recto depth 0: {bot_r_delta}"
    );

    // Top spread (slots 2, 3): inner leaves, depth 1.
    // Top verso (spine_edge Right): shifts right by +0.5 mm.
    let top_v_delta = with_creep[2].content_rect.x - no_creep[2].content_rect.x;
    let expected = mm_to_pt(0.5);
    assert!(
        (top_v_delta - expected).abs() < 0.001,
        "top verso should shift +{expected}, got {top_v_delta}"
    );
    // Top recto (spine_edge Left): shifts left by 0.5 mm (negative delta).
    let top_r_delta = with_creep[3].content_rect.x - no_creep[3].content_rect.x;
    assert!(
        (top_r_delta + expected).abs() < 0.001,
        "top recto should shift -{expected}, got {top_r_delta}"
    );
}

#[test]
fn test_creep_does_not_change_scale_or_size() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = default_margins();
    let cut_edges = calculate_cut_edges(PageArrangement::Folio);
    let slots = build_sheet_slots(
        PageArrangement::Folio,
        leaf_bounds,
        &margins,
        SheetPosition {
            sheet_idx: 0,
            sheets_per_signature: 1,
            sig_start: 0,
        },
        4,
        SheetSide::Front,
    );
    let source_dims = vec![(612.0, 792.0); 4];

    let no_creep = place_slots(
        &slots,
        &cut_edges,
        &source_dims,
        &margins,
        ScalingMode::Fit,
        CreepConfig::None,
    );
    let with_creep = place_slots(
        &slots,
        &cut_edges,
        &source_dims,
        &margins,
        ScalingMode::Fit,
        CreepConfig::PerLayer {
            creep_per_layer_mm: 0.5,
        },
    );

    for (a, b) in no_creep.iter().zip(with_creep.iter()) {
        assert!((a.scale - b.scale).abs() < f32::EPSILON, "scale changed");
        assert!(
            (a.content_rect.width - b.content_rect.width).abs() < f32::EPSILON,
            "width changed"
        );
        assert!(
            (a.content_rect.height - b.content_rect.height).abs() < f32::EPSILON,
            "height changed"
        );
    }
}

#[test]
fn test_inner_sheets_shift_more_than_outer_in_multi_sheet_folio() {
    // In a nested N-sheet folio, the *outermost* physical sheet wraps the
    // bundle and its leaves sit at depth 0 (no shift). Each inner sheet adds a
    // layer of paper at the spine fold, increasing fore-edge protrusion of the
    // leaves it carries — so creep shift grows monotonically from outer to
    // innermost sheet. Both verso and recto on the *same* sheet receive the
    // same shift (the two halves are mirrored around the spine fold).
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = default_margins();
    let cut_edges = calculate_cut_edges(PageArrangement::Folio);
    let source_dims = vec![(612.0, 792.0); 12];
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 0.5,
    };

    let mut verso_x = Vec::new();
    let mut recto_x = Vec::new();
    for sheet_idx in 0..3 {
        let slots = build_sheet_slots(
            PageArrangement::Folio,
            leaf_bounds,
            &margins,
            SheetPosition {
                sheet_idx,
                sheets_per_signature: 3,
                sig_start: 0,
            },
            12,
            SheetSide::Front,
        );
        let placements = place_slots(
            &slots,
            &cut_edges,
            &source_dims,
            &margins,
            ScalingMode::Fit,
            creep,
        );
        verso_x.push(placements[0].content_rect.x);
        recto_x.push(placements[1].content_rect.x);
    }

    // Verso (spine on right): shifts right (larger x) as sheet goes inward.
    assert!(
        verso_x[2] > verso_x[1] && verso_x[1] > verso_x[0],
        "innermost sheet's verso should shift most: {verso_x:?}"
    );
    // Recto (spine on left): shifts left (smaller x) as sheet goes inward.
    assert!(
        recto_x[2] < recto_x[1] && recto_x[1] < recto_x[0],
        "innermost sheet's recto should shift most: {recto_x:?}"
    );
}

#[test]
fn test_folio_verso_and_recto_on_same_sheet_share_depth() {
    // Both halves of a physical folio sheet are at the same nesting depth, so
    // they receive equal-magnitude shifts toward their respective spine edges.
    // Regression for the user-reported asymmetry on 4-sheet folios.
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = default_margins();
    let cut_edges = calculate_cut_edges(PageArrangement::Folio);
    let source_dims = vec![(612.0, 792.0); 16];
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 2.0,
    };

    for sheet_idx in 0..4 {
        let slots = build_sheet_slots(
            PageArrangement::Folio,
            leaf_bounds,
            &margins,
            SheetPosition {
                sheet_idx,
                sheets_per_signature: 4,
                sig_start: 0,
            },
            16,
            SheetSide::Front,
        );
        let no_creep = place_slots(
            &slots,
            &cut_edges,
            &source_dims,
            &margins,
            ScalingMode::Fit,
            CreepConfig::None,
        );
        let with_creep = place_slots(
            &slots,
            &cut_edges,
            &source_dims,
            &margins,
            ScalingMode::Fit,
            creep,
        );

        // Verso shifts right (positive Δx); recto shifts left (negative Δx).
        let verso_delta = with_creep[0].content_rect.x - no_creep[0].content_rect.x;
        let recto_delta = with_creep[1].content_rect.x - no_creep[1].content_rect.x;
        assert!(
            (verso_delta + recto_delta).abs() < 0.001,
            "sheet {sheet_idx}: verso & recto deltas should be equal/opposite, got verso={verso_delta} recto={recto_delta}"
        );
    }
}

#[test]
fn test_signature_boundary_does_not_accumulate() {
    // Each signature is its own nested stack: sheet_idx 0 is always outermost
    // in its signature, regardless of `sig_start`. A regression that globally
    // accumulated sheet_idx across signatures would shift the second
    // signature's outer sheet differently from the first.
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = default_margins();

    let sig0 = build_sheet_slots(
        PageArrangement::Folio,
        leaf_bounds,
        &margins,
        SheetPosition {
            sheet_idx: 0,
            sheets_per_signature: 2,
            sig_start: 0,
        },
        16,
        SheetSide::Front,
    );
    let sig1 = build_sheet_slots(
        PageArrangement::Folio,
        leaf_bounds,
        &margins,
        SheetPosition {
            sheet_idx: 0,
            sheets_per_signature: 2,
            sig_start: 8,
        },
        16,
        SheetSide::Front,
    );

    assert_eq!(sig0[0].leaf_depth, sig1[0].leaf_depth);
    assert_eq!(sig0[1].leaf_depth, sig1[1].leaf_depth);
}
