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
        let expected = (total_leaves - 1) as f32 * 0.1;
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
    // Single-sheet folio: max depth = 1. π·t/2 per layer = π·0.1/2 mm.
    let creep = CreepConfig::FromCaliper {
        paper_thickness_mm: 0.1,
    };
    let actual = max_creep_offset_mm(creep, PageArrangement::Folio, 1);
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
    // Folio single sheet: verso (depth 1) shifts right; recto (depth 0) doesn't shift.
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

    // Verso (slot 0, leaf_depth 1, spine_edge Right): shift right by 0.5 mm.
    let verso_delta = with_creep[0].content_rect.x - no_creep[0].content_rect.x;
    let expected = mm_to_pt(0.5);
    assert!(
        (verso_delta - expected).abs() < 0.001,
        "verso should shift right by {expected}, got {verso_delta}"
    );

    // Recto (slot 1, leaf_depth 0): no shift at depth 0.
    let recto_delta = with_creep[1].content_rect.x - no_creep[1].content_rect.x;
    assert!(
        recto_delta.abs() < 0.001,
        "recto at depth 0 should not shift, got {recto_delta}"
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
fn test_outer_sheet_verso_shifts_most_in_multi_sheet_folio() {
    // Headline property: in a nested N-sheet folio signature, the outermost
    // printed sheet's verso side carries the innermost leaf of the signature,
    // so its verso shift is the maximum. This is the inverse of naive
    // "outer sheet = no shift" reasoning.
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = default_margins();
    let cut_edges = calculate_cut_edges(PageArrangement::Folio);
    let source_dims = vec![(612.0, 792.0); 12];
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 0.5,
    };

    let mut verso_x = Vec::new();
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
    }

    assert!(
        verso_x[0] > verso_x[1],
        "sheet 0 verso should shift more than sheet 1: {verso_x:?}"
    );
    assert!(
        verso_x[1] > verso_x[2],
        "sheet 1 verso should shift more than sheet 2: {verso_x:?}"
    );
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
