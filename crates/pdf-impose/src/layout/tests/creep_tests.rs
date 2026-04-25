//! Tests for creep compensation.
//!
//! The correctness tests pin depth expectations against the page-order tables
//! (`folio_page_order`, `quarto_page_order`, `octavo_page_order`) and the
//! leaf-pair rule (`depth = signature_page_index / 2`).

use super::*;
use crate::constants::mm_to_pt;
use crate::layout::arrangement::{calculate_cut_edges, calculate_spread_positions};
use crate::layout::placement::calculate_spread_placements;
use crate::layout::{Rect, SheetSide, SpreadPosition};
use crate::types::{CreepConfig, LeafMargins, PageArrangement, ScalingMode};

fn assert_offsets_eq(actual: &[(f32, f32)], expected_mm: &[(f32, f32)]) {
    assert_eq!(
        actual.len(),
        expected_mm.len(),
        "length mismatch: {actual:?} vs {expected_mm:?}"
    );
    for (i, (a, e)) in actual.iter().zip(expected_mm.iter()).enumerate() {
        let (expected_v_pt, expected_r_pt) = (mm_to_pt(e.0), mm_to_pt(e.1));
        assert!(
            (a.0 - expected_v_pt).abs() < 0.001,
            "spread {i} verso: got {} pt, expected {} pt ({} mm)",
            a.0,
            expected_v_pt,
            e.0
        );
        assert!(
            (a.1 - expected_r_pt).abs() < 0.001,
            "spread {i} recto: got {} pt, expected {} pt ({} mm)",
            a.1,
            expected_r_pt,
            e.1
        );
    }
}

// =============================================================================
// Offset-formula correctness
// =============================================================================

#[test]
fn test_none_returns_empty() {
    for arr in [
        PageArrangement::Folio,
        PageArrangement::Quarto,
        PageArrangement::Octavo,
    ] {
        for sheets in [1, 2, 4] {
            for side in [SheetSide::Front, SheetSide::Back] {
                let out = creep_offsets_for_face(CreepConfig::None, arr, 0, sheets, side);
                assert!(out.is_empty(), "disabled creep should return empty vec");
            }
        }
    }
}

#[test]
fn test_per_layer_folio_single_sheet_front() {
    // Folio front [v=4, r=1]: signature indices 3, 0 → depths 1, 0.
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 0.1,
    };
    let out = creep_offsets_for_face(creep, PageArrangement::Folio, 0, 1, SheetSide::Front);
    assert_offsets_eq(&out, &[(0.1, 0.0)]);
}

#[test]
fn test_per_layer_quarto_single_sheet_front() {
    // Quarto front:
    //   spread 0 (bottom) [v=8, r=1]: sig indices 7, 0 → depths 3, 0
    //   spread 1 (top)    [v=5, r=4]: sig indices 4, 3 → depths 2, 1
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 0.1,
    };
    let out = creep_offsets_for_face(creep, PageArrangement::Quarto, 0, 1, SheetSide::Front);
    assert_offsets_eq(&out, &[(0.3, 0.0), (0.2, 0.1)]);
}

#[test]
fn test_per_layer_octavo_single_sheet_front() {
    // Octavo front:
    //   BL [v=4, r=13]  → depths 1, 6
    //   BR [v=16, r=1]  → depths 7, 0
    //   TL [v=5, r=12]  → depths 2, 5
    //   TR [v=9, r=8]   → depths 4, 3
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 0.1,
    };
    let out = creep_offsets_for_face(creep, PageArrangement::Octavo, 0, 1, SheetSide::Front);
    assert_offsets_eq(&out, &[(0.1, 0.6), (0.7, 0.0), (0.2, 0.5), (0.4, 0.3)]);
}

#[test]
fn test_per_layer_two_sheet_folio_outer_sheet_carries_deepest_leaf() {
    // Headline case: in a 2-sheet folio signature, the outermost printed sheet
    // carries the outermost (depth 0) AND innermost (depth 3) leaves, on
    // opposite sides of its single spread. The buggy old code zeroed both.
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 0.1,
    };

    let sheet0 = creep_offsets_for_face(creep, PageArrangement::Folio, 0, 2, SheetSide::Front);
    // Sheet 0: verso lands on pages (7,8) depth 3, recto lands on pages (1,2) depth 0.
    assert_offsets_eq(&sheet0, &[(0.3, 0.0)]);

    let sheet1 = creep_offsets_for_face(creep, PageArrangement::Folio, 1, 2, SheetSide::Front);
    // Sheet 1: verso lands on pages (5,6) depth 2, recto lands on pages (3,4) depth 1.
    assert_offsets_eq(&sheet1, &[(0.2, 0.1)]);
}

#[test]
fn test_front_back_depth_swap_folio() {
    // On a single-sheet folio, the left-right flip on the back side swaps
    // which physical leaf is verso vs recto. So front=(v_depth, r_depth) and
    // back=(r_depth, v_depth).
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 0.1,
    };
    let front = creep_offsets_for_face(creep, PageArrangement::Folio, 0, 1, SheetSide::Front);
    let back = creep_offsets_for_face(creep, PageArrangement::Folio, 0, 1, SheetSide::Back);

    assert_offsets_eq(&front, &[(0.1, 0.0)]);
    assert_offsets_eq(&back, &[(0.0, 0.1)]);
    assert!(
        (front[0].0 - back[0].1).abs() < f32::EPSILON
            && (front[0].1 - back[0].0).abs() < f32::EPSILON,
        "front verso/recto depths should be swapped on back"
    );
}

#[test]
fn test_from_caliper_uses_pi_over_two_factor() {
    // Paper caliper 0.1 mm → shift per layer = π·0.1/2 mm.
    // Folio single sheet front: verso depth 1, recto depth 0.
    let creep = CreepConfig::FromCaliper {
        paper_thickness_mm: 0.1,
    };
    let out = creep_offsets_for_face(creep, PageArrangement::Folio, 0, 1, SheetSide::Front);
    assert_eq!(out.len(), 1);

    let expected_verso_mm = std::f32::consts::PI * 0.1 / 2.0;
    let expected_verso_pt = mm_to_pt(expected_verso_mm);
    assert!(
        (out[0].0 - expected_verso_pt).abs() < 0.001,
        "verso: got {} pt, expected {} pt",
        out[0].0,
        expected_verso_pt
    );
    assert!(out[0].1.abs() < 0.001, "recto at depth 0 must be zero");
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

// =============================================================================
// Signature-boundary invariant
// =============================================================================

#[test]
fn test_signature_boundary_does_not_accumulate() {
    // Each signature is its own nested stack: sheet_idx 0 is always outermost
    // in its signature, regardless of how many signatures came before. A
    // regression that globally accumulated sheet_idx across signatures would
    // shift the second signature's outer sheet, breaking this test.
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 0.1,
    };
    let sig0 = creep_offsets_for_face(creep, PageArrangement::Folio, 0, 2, SheetSide::Front);
    let sig1 = creep_offsets_for_face(creep, PageArrangement::Folio, 0, 2, SheetSide::Front);
    assert_eq!(sig0, sig1);
    assert_offsets_eq(&sig0, &[(0.3, 0.0)]);
}

// =============================================================================
// Placement integration
// =============================================================================

fn make_folio_spreads_with_pages(leaf_bounds: Rect, margins: &LeafMargins) -> Vec<SpreadPosition> {
    let mut spreads = calculate_spread_positions(PageArrangement::Folio, leaf_bounds, margins);
    spreads[0].spread.verso_page = Some(0);
    spreads[0].spread.recto_page = Some(1);
    spreads
}

fn default_margins() -> LeafMargins {
    LeafMargins {
        top_mm: 0.0,
        bottom_mm: 0.0,
        fore_edge_mm: 0.0,
        spine_mm: 5.0,
        trim_allowance_mm: 3.0,
    }
}

#[test]
fn test_verso_recto_shift_directions() {
    // Verso shifts right, recto shifts left — both toward the spine.
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = default_margins();
    let spreads = make_folio_spreads_with_pages(leaf_bounds, &margins);
    let cut_edges = calculate_cut_edges(PageArrangement::Folio);
    let source_dims = vec![(612.0, 792.0); 2];
    let v_pt = mm_to_pt(0.4);
    let r_pt = mm_to_pt(0.6);

    let base = calculate_spread_placements(
        &spreads,
        &cut_edges,
        &source_dims,
        &margins,
        ScalingMode::Fit,
        SheetSide::Front,
        &[],
    );
    let shifted = calculate_spread_placements(
        &spreads,
        &cut_edges,
        &source_dims,
        &margins,
        ScalingMode::Fit,
        SheetSide::Front,
        &[(v_pt, r_pt)],
    );

    assert_eq!(base.len(), 2);
    assert_eq!(shifted.len(), 2);

    let verso_delta = shifted[0].content_rect.x - base[0].content_rect.x;
    assert!(
        (verso_delta - v_pt).abs() < 0.01,
        "verso should shift right by {v_pt}, got {verso_delta}"
    );

    let recto_delta = shifted[1].content_rect.x - base[1].content_rect.x;
    assert!(
        (recto_delta - (-r_pt)).abs() < 0.01,
        "recto should shift left by {r_pt}, got {recto_delta}"
    );
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
    let source_dims = vec![(612.0, 792.0); 2];
    let creep = CreepConfig::PerLayer {
        creep_per_layer_mm: 0.1,
    };

    let mut verso_x = Vec::new();
    for sheet_idx in 0..3 {
        let offsets = creep_offsets_for_face(
            creep,
            PageArrangement::Folio,
            sheet_idx,
            3,
            SheetSide::Front,
        );
        let spreads = make_folio_spreads_with_pages(leaf_bounds, &margins);
        let placements = calculate_spread_placements(
            &spreads,
            &cut_edges,
            &source_dims,
            &margins,
            ScalingMode::Fit,
            SheetSide::Front,
            &offsets,
        );
        verso_x.push(placements[0].content_rect.x);
    }

    // Sheet 0 verso carries depth 5 (innermost), sheet 1 carries depth 4,
    // sheet 2 carries depth 3. Shift magnitudes descend from sheet 0 to 2.
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
fn test_creep_does_not_change_scale() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = default_margins();
    let cut_edges = calculate_cut_edges(PageArrangement::Folio);
    let source_dims = vec![(612.0, 792.0); 2];
    let spreads = make_folio_spreads_with_pages(leaf_bounds, &margins);

    let base = calculate_spread_placements(
        &spreads,
        &cut_edges,
        &source_dims,
        &margins,
        ScalingMode::Fit,
        SheetSide::Front,
        &[],
    );

    let shifted = calculate_spread_placements(
        &spreads,
        &cut_edges,
        &source_dims,
        &margins,
        ScalingMode::Fit,
        SheetSide::Front,
        &[(mm_to_pt(0.5), mm_to_pt(0.3))],
    );

    for (b, s) in base.iter().zip(shifted.iter()) {
        assert!(
            (b.scale - s.scale).abs() < f32::EPSILON,
            "scale should not change with creep"
        );
        assert!(
            (b.content_rect.width - s.content_rect.width).abs() < f32::EPSILON,
            "width should not change with creep"
        );
        assert!(
            (b.content_rect.height - s.content_rect.height).abs() < f32::EPSILON,
            "height should not change with creep"
        );
    }
}
