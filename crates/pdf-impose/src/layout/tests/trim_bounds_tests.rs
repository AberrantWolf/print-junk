use super::*;

#[test]
fn test_unified_bounds_from_rects() {
    let rects = vec![
        Rect::new(0.0, 0.0, 100.0, 150.0),
        Rect::new(0.0, 0.0, 120.0, 140.0),
        Rect::new(0.0, 0.0, 90.0, 160.0),
    ];

    let bounds = UnifiedTrimBounds::from_content_rects(&rects);

    assert!((bounds.max_content_width - 120.0).abs() < 0.1);
    assert!((bounds.max_content_height - 160.0).abs() < 0.1);
}

#[test]
fn test_unified_bounds_from_dimensions() {
    let dimensions = vec![(100.0, 150.0), (120.0, 140.0), (90.0, 160.0)];

    let bounds = UnifiedTrimBounds::from_page_dimensions(&dimensions);

    assert!((bounds.max_content_width - 120.0).abs() < 0.1);
    assert!((bounds.max_content_height - 160.0).abs() < 0.1);
}

#[test]
fn test_unified_bounds_empty() {
    let bounds = UnifiedTrimBounds::from_content_rects(&[]);
    assert!(!bounds.is_valid());
}

#[test]
fn test_trim_content_bounds_from_rect() {
    let rect = Rect::new(10.0, 20.0, 100.0, 150.0);
    let bounds = TrimContentBounds::from_rect(&rect);

    assert!((bounds.left - 10.0).abs() < 0.1);
    assert!((bounds.right - 110.0).abs() < 0.1);
    assert!((bounds.bottom - 20.0).abs() < 0.1);
    assert!((bounds.top - 170.0).abs() < 0.1);
}

#[test]
fn test_trim_content_bounds_from_unified() {
    let unified = UnifiedTrimBounds::new(100.0, 150.0);
    let bounds = TrimContentBounds::from_unified(&unified, 200.0, 300.0);

    assert!((bounds.left - 150.0).abs() < 0.1);
    assert!((bounds.right - 250.0).abs() < 0.1);
    assert!((bounds.bottom - 225.0).abs() < 0.1);
    assert!((bounds.top - 375.0).abs() < 0.1);
}

#[test]
fn test_trim_positions_has_cuts() {
    let with_cuts = TrimMarkPositions::new(vec![100.0], vec![], vec![]);
    assert!(with_cuts.has_cuts());

    let no_cuts = TrimMarkPositions::new(vec![], vec![], vec![]);
    assert!(!no_cuts.has_cuts());
}
