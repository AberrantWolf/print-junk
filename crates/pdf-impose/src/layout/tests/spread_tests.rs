use super::*;

#[test]
fn test_folio_spread_creation() {
    let leaf_bounds = Rect::new(25.0, 25.0, 800.0, 600.0);
    let spread = create_folio_spread(leaf_bounds);

    assert!((spread.origin.x - 25.0).abs() < f32::EPSILON);
    assert!((spread.origin.y - 25.0).abs() < f32::EPSILON);
    assert!((spread.width - 800.0).abs() < f32::EPSILON);
    assert!((spread.height - 600.0).abs() < f32::EPSILON);
    assert!(!spread.rotated);
    assert_eq!(spread.spread_index, 0);
}

#[test]
fn test_quarto_spread_creation() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let cut_gap = 10.0;
    let spreads = create_quarto_spreads(leaf_bounds, cut_gap);

    assert_eq!(spreads.len(), 2);

    // Bottom spread
    assert!((spreads[0].origin.y).abs() < f32::EPSILON);
    assert!((spreads[0].height - 295.0).abs() < f32::EPSILON); // (600 - 10) / 2
    assert!(!spreads[0].rotated);

    // Top spread
    assert!((spreads[1].origin.y - 305.0).abs() < f32::EPSILON); // 295 + 10
    assert!((spreads[1].height - 295.0).abs() < f32::EPSILON);
    assert!(spreads[1].rotated);
}

#[test]
fn test_octavo_spread_creation() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let cut_gap_h = 10.0;
    let cut_gap_v = 10.0;
    let spreads = create_octavo_spreads(leaf_bounds, cut_gap_h, cut_gap_v);

    assert_eq!(spreads.len(), 4);

    let half_width = (800.0 - 10.0) / 2.0; // 395
    let half_height = (600.0 - 10.0) / 2.0; // 295

    // Bottom-left
    assert!((spreads[0].origin.x).abs() < f32::EPSILON);
    assert!((spreads[0].origin.y).abs() < f32::EPSILON);
    assert!((spreads[0].width - half_width).abs() < f32::EPSILON);
    assert!(!spreads[0].rotated);

    // Bottom-right
    assert!((spreads[1].origin.x - (half_width + 10.0)).abs() < f32::EPSILON);
    assert!((spreads[1].origin.y).abs() < f32::EPSILON);
    assert!(!spreads[1].rotated);

    // Top-left
    assert!((spreads[2].origin.x).abs() < f32::EPSILON);
    assert!((spreads[2].origin.y - (half_height + 10.0)).abs() < f32::EPSILON);
    assert!(spreads[2].rotated);

    // Top-right
    assert!((spreads[3].origin.x - (half_width + 10.0)).abs() < f32::EPSILON);
    assert!((spreads[3].origin.y - (half_height + 10.0)).abs() < f32::EPSILON);
    assert!(spreads[3].rotated);
}

// Per-slot content-rect tests live in `slots_tests.rs` (the new home of the
// margin math). The old `calculate_spread_content` paired path is gone.
