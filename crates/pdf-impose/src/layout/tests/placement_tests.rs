use super::*;
use crate::layout::arrangement::{calculate_cut_edges, calculate_spread_positions};
use crate::layout::spread::calculate_spread_content;
use crate::layout::{Point, SpreadCutEdges, SpreadPosition};
use crate::types::{LeafMargins, PageArrangement};

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
// Non-Uniform Margin Uniformity Tests
// =============================================================================

/// Within a single spread, verso and recto pages should have the same dimensions
/// even when spine != `fore_edge` and top != bottom.
#[test]
fn test_nonuniform_margins_verso_recto_same_size() {
    let spread_pos = SpreadPosition::empty(Point::new(0.0, 0.0), 600.0, 400.0, false, 0);
    let margins = nonuniform_margins();

    // Test with no cut edges
    let content = calculate_spread_content(&spread_pos, &margins, SpreadCutEdges::none());
    assert!(
        (content.verso.width - content.recto.width).abs() < 0.01,
        "Verso and recto widths should match: {} vs {}",
        content.verso.width,
        content.recto.width
    );
    assert!(
        (content.verso.height - content.recto.height).abs() < 0.01,
        "Verso and recto heights should match: {} vs {}",
        content.verso.height,
        content.recto.height
    );

    // Test with symmetric cut edges (both left and right)
    let cuts = SpreadCutEdges {
        top: true,
        bottom: false,
        left: true,
        right: true,
    };
    let content = calculate_spread_content(&spread_pos, &margins, cuts);
    assert!(
        (content.verso.width - content.recto.width).abs() < 0.01,
        "With symmetric cuts, verso and recto widths should match: {} vs {}",
        content.verso.width,
        content.recto.width
    );
    assert!(
        (content.verso.height - content.recto.height).abs() < 0.01,
        "With symmetric cuts, verso and recto heights should match",
    );
}

/// In quarto (2 spreads stacked), all 4 page content areas should have the
/// same dimensions despite different cut edge configurations per spread.
#[test]
fn test_nonuniform_margins_quarto_spreads_same_page_size() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = nonuniform_margins();

    let spreads = calculate_spread_positions(PageArrangement::Quarto, leaf_bounds, &margins);
    let cut_edges = calculate_cut_edges(PageArrangement::Quarto);

    let mut all_widths = Vec::new();
    let mut all_heights = Vec::new();

    for (spread, cuts) in spreads.iter().zip(cut_edges.iter()) {
        let content = calculate_spread_content(spread, &margins, *cuts);
        all_widths.push(content.verso.width);
        all_widths.push(content.recto.width);
        all_heights.push(content.verso.height);
        all_heights.push(content.recto.height);
    }

    // All widths should be the same
    let first_width = all_widths[0];
    for (i, &w) in all_widths.iter().enumerate() {
        assert!(
            (w - first_width).abs() < 0.01,
            "Quarto page {i} width {w} differs from first {first_width}"
        );
    }

    // All heights should be the same
    let first_height = all_heights[0];
    for (i, &h) in all_heights.iter().enumerate() {
        assert!(
            (h - first_height).abs() < 0.01,
            "Quarto page {i} height {h} differs from first {first_height}"
        );
    }
}

/// In octavo (4 spreads in 2x2), all 8 page content areas should have the
/// same dimensions. The cut margin is applied symmetrically — when a spread
/// has a vertical cut on either side, both pages get the cut margin on their
/// fore-edge so all pages end up the same width.
#[test]
fn test_nonuniform_margins_octavo_spreads_same_page_size() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = nonuniform_margins();

    let spreads = calculate_spread_positions(PageArrangement::Octavo, leaf_bounds, &margins);
    let cut_edges = calculate_cut_edges(PageArrangement::Octavo);

    let mut all_widths = Vec::new();
    let mut all_heights = Vec::new();

    for (spread, cuts) in spreads.iter().zip(cut_edges.iter()) {
        let content = calculate_spread_content(spread, &margins, *cuts);
        all_widths.push(content.verso.width);
        all_widths.push(content.recto.width);
        all_heights.push(content.verso.height);
        all_heights.push(content.recto.height);
    }

    let first_width = all_widths[0];
    for (i, &w) in all_widths.iter().enumerate() {
        assert!(
            (w - first_width).abs() < 0.01,
            "Octavo page {i} width {w} differs from first {first_width}"
        );
    }

    let first_height = all_heights[0];
    for (i, &h) in all_heights.iter().enumerate() {
        assert!(
            (h - first_height).abs() < 0.01,
            "Octavo page {i} height {h} differs from first {first_height}"
        );
    }
}

/// End-to-end: uniform source pages through `calculate_spread_placements` with
/// non-uniform margins should produce identical scale factors and content rect
/// dimensions for all pages.
#[test]
fn test_nonuniform_margins_placements_uniform_scale() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = nonuniform_margins();

    // Use quarto as it has cut edges
    let arrangement = PageArrangement::Quarto;
    let mut spreads = calculate_spread_positions(arrangement, leaf_bounds, &margins);
    let cut_edges = calculate_cut_edges(arrangement);

    // Assign pages to spreads (8 pages for quarto)
    for (i, spread) in spreads.iter_mut().enumerate() {
        spread.spread.verso_page = Some(i * 2);
        spread.spread.recto_page = Some(i * 2 + 1);
    }

    // All source pages are 612x792 (US Letter)
    let source_dims: Vec<(f32, f32)> = vec![(612.0, 792.0); 8];

    let placements = calculate_spread_placements(
        &spreads,
        &cut_edges,
        &source_dims,
        &margins,
        ScalingMode::Fit,
        SheetSide::Front,
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
            "Placement {} scale {} differs from first {}",
            i,
            p.scale,
            first_scale
        );
        assert!(
            (p.content_rect.width - first_width).abs() < 0.1,
            "Placement {} width {} differs from first {}",
            i,
            p.content_rect.width,
            first_width
        );
        assert!(
            (p.content_rect.height - first_height).abs() < 0.1,
            "Placement {} height {} differs from first {}",
            i,
            p.content_rect.height,
            first_height
        );
    }
}
