use super::*;

fn default_margins() -> LeafMargins {
    LeafMargins {
        top_mm: 10.0,
        bottom_mm: 10.0,
        fore_edge_mm: 15.0,
        spine_mm: 20.0,
        trim_allowance_mm: 5.0,
    }
}

#[test]
fn test_folio_spread_creation() {
    let leaf_bounds = Rect::new(25.0, 25.0, 800.0, 600.0);
    let spread = create_folio_spread(leaf_bounds);

    assert_eq!(spread.origin.x, 25.0);
    assert_eq!(spread.origin.y, 25.0);
    assert_eq!(spread.width, 800.0);
    assert_eq!(spread.height, 600.0);
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
    assert_eq!(spreads[0].origin.y, 0.0);
    assert_eq!(spreads[0].height, 295.0); // (600 - 10) / 2
    assert!(!spreads[0].rotated);

    // Top spread
    assert_eq!(spreads[1].origin.y, 305.0); // 295 + 10
    assert_eq!(spreads[1].height, 295.0);
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
    assert_eq!(spreads[0].origin.x, 0.0);
    assert_eq!(spreads[0].origin.y, 0.0);
    assert_eq!(spreads[0].width, half_width);
    assert!(!spreads[0].rotated);

    // Bottom-right
    assert_eq!(spreads[1].origin.x, half_width + 10.0);
    assert_eq!(spreads[1].origin.y, 0.0);
    assert!(!spreads[1].rotated);

    // Top-left
    assert_eq!(spreads[2].origin.x, 0.0);
    assert_eq!(spreads[2].origin.y, half_height + 10.0);
    assert!(spreads[2].rotated);

    // Top-right
    assert_eq!(spreads[3].origin.x, half_width + 10.0);
    assert_eq!(spreads[3].origin.y, half_height + 10.0);
    assert!(spreads[3].rotated);
}

#[test]
fn test_spread_content_no_cuts() {
    let spread_pos = SpreadPosition::empty(
        Point::new(0.0, 0.0),
        400.0, // 200 per page
        300.0,
        false,
        0,
    );
    let margins = default_margins();
    let cut_edges = SpreadCutEdges::none();

    let content = calculate_spread_content(&spread_pos, &margins, cut_edges);

    // Verso: x = fore_edge, width = 200 - fore_edge - spine
    let fore_edge_pt = mm_to_pt(15.0);
    let spine_pt = mm_to_pt(20.0);
    let top_pt = mm_to_pt(10.0);
    let bottom_pt = mm_to_pt(10.0);

    assert!((content.verso.x - fore_edge_pt).abs() < 0.1);
    assert!((content.verso.width - (200.0 - fore_edge_pt - spine_pt)).abs() < 0.1);

    // Recto: x = 200 + spine, width = 200 - spine - fore_edge
    assert!((content.recto.x - (200.0 + spine_pt)).abs() < 0.1);
    assert!((content.recto.width - (200.0 - spine_pt - fore_edge_pt)).abs() < 0.1);

    // Both pages have same height
    let expected_height = 300.0 - top_pt - bottom_pt;
    assert!((content.verso.height - expected_height).abs() < 0.1);
    assert!((content.recto.height - expected_height).abs() < 0.1);
}

#[test]
fn test_spread_content_with_cuts() {
    let spread_pos = SpreadPosition::empty(Point::new(0.0, 0.0), 400.0, 300.0, false, 0);
    let margins = default_margins();
    let cut_edges = SpreadCutEdges {
        top: true,
        bottom: false,
        left: true,
        right: false,
    };

    let content = calculate_spread_content(&spread_pos, &margins, cut_edges);

    // With cut on left, verso uses fore_edge + cut margin
    let fore_edge_pt = mm_to_pt(15.0);
    let cut_pt = mm_to_pt(5.0);
    assert!((content.verso.x - (fore_edge_pt + cut_pt)).abs() < 0.1);

    // With cut on top, both pages have reduced height
    let top_pt = mm_to_pt(10.0);
    let bottom_pt = mm_to_pt(10.0);
    let expected_height = 300.0 - (top_pt + cut_pt) - bottom_pt;
    assert!((content.verso.height - expected_height).abs() < 0.1);
}
