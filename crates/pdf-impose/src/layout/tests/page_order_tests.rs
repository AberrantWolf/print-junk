use super::*;

#[test]
fn test_folio_page_order() {
    let (front, back) = page_order_for_arrangement(PageArrangement::Folio);

    // Front: verso=4 (idx 3), recto=1 (idx 0)
    assert_eq!(front, vec![3, 0]);
    // Back: verso=2 (idx 1), recto=3 (idx 2)
    assert_eq!(back, vec![1, 2]);
}

#[test]
fn test_folio_assignment() {
    let sheets = assign_pages_to_spreads(PageArrangement::Folio, 1, 0, 4);
    assert_eq!(sheets.len(), 1);
    let assignment = &sheets[0];

    // Front side: 1 spread with pages 4, 1
    assert_eq!(assignment.front.len(), 1);
    assert_eq!(assignment.front[0].verso_page, Some(3)); // page 4
    assert_eq!(assignment.front[0].recto_page, Some(0)); // page 1

    // Back side: 1 spread with pages 2, 3
    assert_eq!(assignment.back.len(), 1);
    assert_eq!(assignment.back[0].verso_page, Some(1)); // page 2
    assert_eq!(assignment.back[0].recto_page, Some(2)); // page 3
}

#[test]
fn test_folio_assignment_with_blanks() {
    // Only 2 source pages, but folio needs 4
    let sheets = assign_pages_to_spreads(PageArrangement::Folio, 1, 0, 2);
    let assignment = &sheets[0];

    // Front: verso=4 (blank), recto=1 (page 0)
    assert_eq!(assignment.front[0].verso_page, None);
    assert_eq!(assignment.front[0].recto_page, Some(0));

    // Back: verso=2 (page 1), recto=3 (blank)
    assert_eq!(assignment.back[0].verso_page, Some(1));
    assert_eq!(assignment.back[0].recto_page, None);
}

#[test]
fn test_quarto_page_order() {
    let (front, back) = page_order_for_arrangement(PageArrangement::Quarto);

    // Front: [bottom: 8,1], [top: 5,4]
    assert_eq!(front, vec![7, 0, 4, 3]);
    // Back: [bottom: 2,7], [top: 3,6]
    assert_eq!(back, vec![1, 6, 2, 5]);
}

#[test]
fn test_octavo_page_order() {
    let (front, back) = page_order_for_arrangement(PageArrangement::Octavo);

    // Front (bottom row + top row, top rotated):
    //   BL [v=4, r=13], BR [v=16, r=1], TL [v=5, r=12], TR [v=9, r=8]
    assert_eq!(front, vec![3, 12, 15, 0, 4, 11, 8, 7]);
    // Back (mirror each row, step to leaf-mate):
    //   BL [v=2, r=15], BR [v=14, r=3], TL [v=7, r=10], TR [v=11, r=6]
    assert_eq!(back, vec![1, 14, 13, 2, 6, 9, 10, 5]);
}

#[test]
fn test_derive_back_matches_known_tables() {
    // Pin the derivation against the previously hand-derived back tables.
    // If the leaf-pair flip rule changes, this test catches it before the
    // assignment tests do.
    for (arrangement, expected_back) in [
        (PageArrangement::Folio, vec![1, 2]),
        (PageArrangement::Quarto, vec![1, 6, 2, 5]),
        (PageArrangement::Octavo, vec![1, 14, 13, 2, 6, 9, 10, 5]),
    ] {
        let (_, back) = page_order_for_arrangement(arrangement);
        assert_eq!(back, expected_back, "arrangement={arrangement:?}");
    }
}

#[test]
fn test_quarto_assignment() {
    let sheets = assign_pages_to_spreads(PageArrangement::Quarto, 1, 0, 8);
    let assignment = &sheets[0];

    // Front has 2 spreads
    assert_eq!(assignment.front.len(), 2);

    // Bottom spread: verso=8, recto=1
    assert_eq!(assignment.front[0].verso_page, Some(7));
    assert_eq!(assignment.front[0].recto_page, Some(0));

    // Top spread: verso=5, recto=4
    assert_eq!(assignment.front[1].verso_page, Some(4));
    assert_eq!(assignment.front[1].recto_page, Some(3));

    // Back has 2 spreads
    assert_eq!(assignment.back.len(), 2);

    // Bottom spread: verso=2, recto=7
    assert_eq!(assignment.back[0].verso_page, Some(1));
    assert_eq!(assignment.back[0].recto_page, Some(6));

    // Top spread: verso=3, recto=6
    assert_eq!(assignment.back[1].verso_page, Some(2));
    assert_eq!(assignment.back[1].recto_page, Some(5));
}

#[test]
fn test_octavo_assignment() {
    let sheets = assign_pages_to_spreads(PageArrangement::Octavo, 1, 0, 16);
    let assignment = &sheets[0];

    // Front has 4 spreads
    assert_eq!(assignment.front.len(), 4);

    // Bottom-left: verso=4, recto=13
    assert_eq!(assignment.front[0].verso_page, Some(3));
    assert_eq!(assignment.front[0].recto_page, Some(12));

    // Bottom-right: verso=16, recto=1
    assert_eq!(assignment.front[1].verso_page, Some(15));
    assert_eq!(assignment.front[1].recto_page, Some(0));

    // Top-left: verso=5, recto=12
    assert_eq!(assignment.front[2].verso_page, Some(4));
    assert_eq!(assignment.front[2].recto_page, Some(11));

    // Top-right: verso=9, recto=8
    assert_eq!(assignment.front[3].verso_page, Some(8));
    assert_eq!(assignment.front[3].recto_page, Some(7));

    // Back has 4 spreads
    assert_eq!(assignment.back.len(), 4);
}

#[test]
fn test_second_signature() {
    // Second folio signature starts at page 4
    let sheets = assign_pages_to_spreads(PageArrangement::Folio, 1, 4, 8);
    let assignment = &sheets[0];

    // Front: pages 8, 5 (indices 7, 4)
    assert_eq!(assignment.front[0].verso_page, Some(7));
    assert_eq!(assignment.front[0].recto_page, Some(4));

    // Back: pages 6, 7 (indices 5, 6)
    assert_eq!(assignment.back[0].verso_page, Some(5));
    assert_eq!(assignment.back[0].recto_page, Some(6));
}

#[test]
fn test_signature_count() {
    // 4 pages needs 1 folio signature
    assert_eq!(calculate_signature_count(4, 4), 1);

    // 5 pages needs 2 folio signatures
    assert_eq!(calculate_signature_count(5, 4), 2);

    // 16 pages needs 1 octavo signature
    assert_eq!(calculate_signature_count(16, 16), 1);

    // 17 pages needs 2 octavo signatures
    assert_eq!(calculate_signature_count(17, 16), 2);
}

#[test]
fn test_padded_page_count() {
    // 3 pages padded to folio = 4
    assert_eq!(calculate_padded_page_count(3, 4), 4);

    // 12 pages padded to octavo = 16
    assert_eq!(calculate_padded_page_count(12, 16), 16);

    // 17 pages padded to octavo = 32
    assert_eq!(calculate_padded_page_count(17, 16), 32);
}

#[test]
fn test_apply_page_assignments() {
    use super::super::Point;

    let positions = vec![
        SpreadPosition::empty(Point::new(0.0, 0.0), 400.0, 300.0, false, 0),
        SpreadPosition::empty(Point::new(0.0, 310.0), 400.0, 300.0, true, 1),
    ];

    let spreads = vec![Spread::new(Some(7), Some(0)), Spread::new(Some(4), Some(3))];

    let result = apply_page_assignments(&positions, &spreads);

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].spread.verso_page, Some(7));
    assert_eq!(result[0].spread.recto_page, Some(0));
    assert!(!result[0].rotated);

    assert_eq!(result[1].spread.verso_page, Some(4));
    assert_eq!(result[1].spread.recto_page, Some(3));
    assert!(result[1].rotated);
}

// =============================================================================
// Multi-sheet nesting tests
// =============================================================================

#[test]
fn test_nesting_remap_single_sheet() {
    // Single sheet: identity mapping
    let remap = build_nesting_remap(0, 1, 4);
    assert_eq!(remap, vec![0, 1, 2, 3]);

    let remap = build_nesting_remap(0, 1, 8);
    assert_eq!(remap, vec![0, 1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn test_nesting_remap_folio_3_sheets() {
    // 3 folio sheets = 12 pages total
    // Outer sheet (0): pages 0,1 and 10,11
    let remap = build_nesting_remap(0, 3, 4);
    assert_eq!(remap, vec![0, 1, 10, 11]);

    // Middle sheet (1): pages 2,3 and 8,9
    let remap = build_nesting_remap(1, 3, 4);
    assert_eq!(remap, vec![2, 3, 8, 9]);

    // Inner sheet (2): pages 4,5 and 6,7
    let remap = build_nesting_remap(2, 3, 4);
    assert_eq!(remap, vec![4, 5, 6, 7]);
}

#[test]
fn test_nesting_remap_quarto_2_sheets() {
    // 2 quarto sheets = 16 pages total
    // Outer sheet (0): pages 0-3 and 12-15
    let remap = build_nesting_remap(0, 2, 8);
    assert_eq!(remap, vec![0, 1, 2, 3, 12, 13, 14, 15]);

    // Inner sheet (1): pages 4-7 and 8-11
    let remap = build_nesting_remap(1, 2, 8);
    assert_eq!(remap, vec![4, 5, 6, 7, 8, 9, 10, 11]);
}

#[test]
fn test_multi_sheet_folio_assignment() {
    // 3 folio sheets, 12 pages
    let sheets = assign_pages_to_spreads(PageArrangement::Folio, 3, 0, 12);
    assert_eq!(sheets.len(), 3);

    // Outer sheet: folio page order applied to pages {0,1,10,11}
    // Folio order: front=[verso=3, recto=0], back=[verso=1, recto=2]
    // Remapped: front=[verso=11, recto=0], back=[verso=1, recto=10]
    assert_eq!(sheets[0].front[0].verso_page, Some(11));
    assert_eq!(sheets[0].front[0].recto_page, Some(0));
    assert_eq!(sheets[0].back[0].verso_page, Some(1));
    assert_eq!(sheets[0].back[0].recto_page, Some(10));

    // Inner sheet: pages {4,5,6,7}
    // Remapped: front=[verso=7, recto=4], back=[verso=5, recto=6]
    assert_eq!(sheets[2].front[0].verso_page, Some(7));
    assert_eq!(sheets[2].front[0].recto_page, Some(4));
    assert_eq!(sheets[2].back[0].verso_page, Some(5));
    assert_eq!(sheets[2].back[0].recto_page, Some(6));
}
