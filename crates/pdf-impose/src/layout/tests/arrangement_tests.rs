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
fn test_arrangement_config_folio() {
    let config = ArrangementConfig::for_arrangement(PageArrangement::Folio);
    assert_eq!(config.cols, 1);
    assert_eq!(config.rows, 1);
    assert_eq!(config.spread_count, 1);
    assert_eq!(config.pages_per_signature, 4);
}

#[test]
fn test_arrangement_config_quarto() {
    let config = ArrangementConfig::for_arrangement(PageArrangement::Quarto);
    assert_eq!(config.cols, 1);
    assert_eq!(config.rows, 2);
    assert_eq!(config.spread_count, 2);
    assert_eq!(config.pages_per_signature, 8);
}

#[test]
fn test_arrangement_config_octavo() {
    let config = ArrangementConfig::for_arrangement(PageArrangement::Octavo);
    assert_eq!(config.cols, 2);
    assert_eq!(config.rows, 2);
    assert_eq!(config.spread_count, 4);
    assert_eq!(config.pages_per_signature, 16);
}

#[test]
fn test_folio_spread_positions() {
    let leaf_bounds = Rect::new(25.0, 25.0, 800.0, 600.0);
    let margins = default_margins();

    let spreads = calculate_spread_positions(PageArrangement::Folio, leaf_bounds, &margins);

    assert_eq!(spreads.len(), 1);
    assert_eq!(spreads[0].width, 800.0);
    assert_eq!(spreads[0].height, 600.0);
    assert!(!spreads[0].rotated);
}

#[test]
fn test_quarto_spread_positions() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = default_margins();

    let spreads = calculate_spread_positions(PageArrangement::Quarto, leaf_bounds, &margins);

    assert_eq!(spreads.len(), 2);

    // Both spreads have full width
    assert_eq!(spreads[0].width, 800.0);
    assert_eq!(spreads[1].width, 800.0);

    // Bottom spread is not rotated
    assert!(!spreads[0].rotated);
    // Top spread is rotated
    assert!(spreads[1].rotated);

    // Check heights account for cut gap
    let cut_gap = mm_to_pt(margins.trim_allowance_mm);
    let expected_height = (600.0 - cut_gap) / 2.0;
    assert!((spreads[0].height - expected_height).abs() < 0.1);
    assert!((spreads[1].height - expected_height).abs() < 0.1);
}

#[test]
fn test_octavo_spread_positions() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let margins = default_margins();

    let spreads = calculate_spread_positions(PageArrangement::Octavo, leaf_bounds, &margins);

    assert_eq!(spreads.len(), 4);

    // Bottom row not rotated, top row rotated
    assert!(!spreads[0].rotated); // bottom-left
    assert!(!spreads[1].rotated); // bottom-right
    assert!(spreads[2].rotated); // top-left
    assert!(spreads[3].rotated); // top-right

    // Check spread indices
    assert_eq!(spreads[0].spread_index, 0);
    assert_eq!(spreads[1].spread_index, 1);
    assert_eq!(spreads[2].spread_index, 2);
    assert_eq!(spreads[3].spread_index, 3);
}

#[test]
fn test_cut_edges_folio() {
    let edges = calculate_cut_edges(PageArrangement::Folio);
    assert_eq!(edges.len(), 1);
    assert!(!edges[0].any());
}

#[test]
fn test_cut_edges_quarto() {
    let edges = calculate_cut_edges(PageArrangement::Quarto);
    assert_eq!(edges.len(), 2);

    // Bottom spread: cut above (top = true)
    assert!(edges[0].top);
    assert!(!edges[0].bottom);

    // Top spread: cut below (bottom = true)
    assert!(!edges[1].top);
    assert!(edges[1].bottom);
}

#[test]
fn test_cut_edges_octavo() {
    let edges = calculate_cut_edges(PageArrangement::Octavo);
    assert_eq!(edges.len(), 4);

    // Bottom-left: cut above, cut right
    assert!(edges[0].top);
    assert!(!edges[0].bottom);
    assert!(!edges[0].left);
    assert!(edges[0].right);

    // Bottom-right: cut above, cut left
    assert!(edges[1].top);
    assert!(!edges[1].bottom);
    assert!(edges[1].left);
    assert!(!edges[1].right);

    // Top-left: cut below, cut right
    assert!(!edges[2].top);
    assert!(edges[2].bottom);
    assert!(!edges[2].left);
    assert!(edges[2].right);

    // Top-right: cut below, cut left
    assert!(!edges[3].top);
    assert!(edges[3].bottom);
    assert!(edges[3].left);
    assert!(!edges[3].right);
}

#[test]
fn test_cut_positions_folio() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let cuts = CutPositions::for_arrangement(PageArrangement::Folio, &leaf_bounds, 5.0);
    assert!(!cuts.any());
}

#[test]
fn test_cut_positions_quarto() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let cuts = CutPositions::for_arrangement(PageArrangement::Quarto, &leaf_bounds, 5.0);

    assert!(cuts.vertical.is_empty());
    assert_eq!(cuts.horizontal.len(), 1);
    assert!((cuts.horizontal[0] - 300.0).abs() < 0.1);
}

#[test]
fn test_cut_positions_octavo() {
    let leaf_bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
    let cuts = CutPositions::for_arrangement(PageArrangement::Octavo, &leaf_bounds, 5.0);

    assert_eq!(cuts.vertical.len(), 1);
    assert_eq!(cuts.horizontal.len(), 1);
    assert!((cuts.vertical[0] - 400.0).abs() < 0.1);
    assert!((cuts.horizontal[0] - 300.0).abs() < 0.1);
}

#[test]
fn test_cut_edges_three_rows() {
    // 3 rows, 1 col = 3 spreads stacked vertically
    // Validates correctness for layouts with middle rows
    let config = ArrangementConfig {
        cols: 1,
        rows: 3,
        spread_count: 3,
        pages_per_signature: 12,
    };
    let edges: Vec<_> = (0..config.spread_count)
        .map(|i| spread_cut_edges(i, config.cols, config.rows))
        .collect();

    // Bottom row (row 0): cut above only
    assert!(edges[0].top);
    assert!(!edges[0].bottom);

    // Middle row (row 1): cuts above AND below
    assert!(edges[1].top);
    assert!(edges[1].bottom);

    // Top row (row 2): cut below only
    assert!(!edges[2].top);
    assert!(edges[2].bottom);
}
