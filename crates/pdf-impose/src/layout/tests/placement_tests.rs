use super::*;
use crate::layout::SheetSide;
use crate::layout::arrangement::calculate_cut_edges;
use crate::layout::slots::{SheetPosition, build_sheet_slots, slot_content_rect};
use crate::types::{CreepConfig, LeafMargins, PageArrangement};

fn nonuniform_margins() -> LeafMargins {
    LeafMargins {
        top_mm: 8.0,
        bottom_mm: 12.0,
        fore_edge_mm: 10.0,
        spine_mm: 18.0,
        trim_allowance_mm: 4.0,
    }
}

#[test]
fn test_scaling_modes() {
    // Test fit mode
    let scale = calculate_scale(800.0, 600.0, 400.0, 400.0, ScalingMode::Fit);
    assert!((scale - 0.5).abs() < 0.001, "Fit should use smaller scale");

    // Test fill mode
    let scale = calculate_scale(800.0, 600.0, 400.0, 400.0, ScalingMode::Fill);
    assert!(
        (scale - 400.0 / 600.0).abs() < 0.001,
        "Fill should use larger scale"
    );

    // Test none mode
    let scale = calculate_scale(800.0, 600.0, 400.0, 400.0, ScalingMode::None);
    assert!((scale - 1.0).abs() < 0.001, "None should return 1.0");
}

// =============================================================================
// Scale Guard Tests
// =============================================================================

#[test]
fn test_scale_zero_source_dimensions() {
    let scale = calculate_scale(0.0, 600.0, 400.0, 400.0, ScalingMode::Fit);
    assert!(
        (scale - 1.0).abs() < 0.001,
        "Zero source width should return 1.0"
    );

    let scale = calculate_scale(800.0, 0.0, 400.0, 400.0, ScalingMode::Fit);
    assert!(
        (scale - 1.0).abs() < 0.001,
        "Zero source height should return 1.0"
    );

    let scale = calculate_scale(0.0, 0.0, 400.0, 400.0, ScalingMode::Fill);
    assert!(
        (scale - 1.0).abs() < 0.001,
        "Zero source both should return 1.0"
    );
}

#[test]
fn test_scale_negative_source_dimensions() {
    let scale = calculate_scale(-100.0, 600.0, 400.0, 400.0, ScalingMode::Fit);
    assert!(
        (scale - 1.0).abs() < 0.001,
        "Negative source width should return 1.0"
    );
}

#[test]
fn test_scale_zero_target_dimensions() {
    let scale = calculate_scale(800.0, 600.0, 0.0, 400.0, ScalingMode::Fit);
    assert!(
        (scale - 1.0).abs() < 0.001,
        "Zero target width should return 1.0"
    );

    let scale = calculate_scale(800.0, 600.0, 400.0, 0.0, ScalingMode::Fit);
    assert!(
        (scale - 1.0).abs() < 0.001,
        "Zero target height should return 1.0"
    );
}

// =============================================================================
// End-to-end placement scale uniformity
// =============================================================================

/// Uniform source pages through `place_slots` with non-uniform margins should
/// produce identical scale factors and content-rect dimensions for every page.
#[test]
fn test_nonuniform_margins_placements_uniform_scale() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = nonuniform_margins();

    // Quarto exercises both no-cut and cut edges.
    let arrangement = PageArrangement::Quarto;
    let cut_edges = calculate_cut_edges(arrangement);
    let slots = build_sheet_slots(
        arrangement,
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

    // All source pages are 612×792 (US Letter)
    let source_dims: Vec<(f32, f32)> = vec![(612.0, 792.0); 8];

    let placements = place_slots(
        &slots,
        &cut_edges,
        &source_dims,
        &margins,
        ScalingMode::Fit,
        CreepConfig::None,
    );

    assert_eq!(
        placements.len(),
        4,
        "Should have 4 placements for quarto front"
    );

    let first_scale = placements[0].scale;
    let first_width = placements[0].content_rect.width;
    let first_height = placements[0].content_rect.height;

    for (i, p) in placements.iter().enumerate() {
        assert!(
            (p.scale - first_scale).abs() < 0.001,
            "Placement {i} scale {} differs from first {first_scale}",
            p.scale,
        );
        assert!(
            (p.content_rect.width - first_width).abs() < 0.1,
            "Placement {i} width {} differs from first {first_width}",
            p.content_rect.width,
        );
        assert!(
            (p.content_rect.height - first_height).abs() < 0.1,
            "Placement {i} height {} differs from first {first_height}",
            p.content_rect.height,
        );
    }
}

// =============================================================================
// Per-slot content-rect uniformity (replaces the old spread-content tests)
// =============================================================================

/// In quarto (2 spreads stacked), all 4 page content areas should have the
/// same dimensions despite different cut-edge configurations per spread.
#[test]
fn test_nonuniform_margins_quarto_slots_same_page_size() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = nonuniform_margins();

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
    let cut_edges = calculate_cut_edges(PageArrangement::Quarto);

    let dims: Vec<(f32, f32)> = slots
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            let r = slot_content_rect(slot, &margins, cut_edges[i / 2]);
            (r.width, r.height)
        })
        .collect();

    let (first_w, first_h) = dims[0];
    for (i, &(w, h)) in dims.iter().enumerate() {
        assert!(
            (w - first_w).abs() < 0.01,
            "Quarto slot {i} width {w} differs from first {first_w}"
        );
        assert!(
            (h - first_h).abs() < 0.01,
            "Quarto slot {i} height {h} differs from first {first_h}"
        );
    }
}
