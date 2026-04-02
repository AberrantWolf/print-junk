use super::*;

#[test]
fn test_folio_page_order() {
    let (front, back) = folio_page_order();

    // Front: verso=4 (idx 3), recto=1 (idx 0)
    assert_eq!(front, vec![3, 0]);
    // Back: verso=2 (idx 1), recto=3 (idx 2)
    assert_eq!(back, vec![1, 2]);
}

#[test]
fn test_folio_assignment() {
    let assignment = assign_pages_to_spreads(PageArrangement::Folio, 0, 4);

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
    let assignment = assign_pages_to_spreads(PageArrangement::Folio, 0, 2);

    // Front: verso=4 (blank), recto=1 (page 0)
    assert_eq!(assignment.front[0].verso_page, None);
    assert_eq!(assignment.front[0].recto_page, Some(0));

    // Back: verso=2 (page 1), recto=3 (blank)
    assert_eq!(assignment.back[0].verso_page, Some(1));
    assert_eq!(assignment.back[0].recto_page, None);
}

#[test]
fn test_quarto_page_order() {
    let (front, back) = quarto_page_order();

    // Front: [bottom: 8,1], [top: 5,4]
    assert_eq!(front, vec![7, 0, 4, 3]);
    // Back: [bottom: 2,7], [top: 3,6]
    assert_eq!(back, vec![1, 6, 2, 5]);
}

#[test]
fn test_quarto_assignment() {
    let assignment = assign_pages_to_spreads(PageArrangement::Quarto, 0, 8);

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
    let assignment = assign_pages_to_spreads(PageArrangement::Octavo, 0, 16);

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
    let assignment = assign_pages_to_spreads(PageArrangement::Folio, 4, 8);

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
    assert_eq!(calculate_signature_count(4, PageArrangement::Folio), 1);

    // 5 pages needs 2 folio signatures
    assert_eq!(calculate_signature_count(5, PageArrangement::Folio), 2);

    // 16 pages needs 1 octavo signature
    assert_eq!(calculate_signature_count(16, PageArrangement::Octavo), 1);

    // 17 pages needs 2 octavo signatures
    assert_eq!(calculate_signature_count(17, PageArrangement::Octavo), 2);
}

#[test]
fn test_padded_page_count() {
    // 3 pages padded to folio = 4
    assert_eq!(calculate_padded_page_count(3, PageArrangement::Folio), 4);

    // 12 pages padded to octavo = 16
    assert_eq!(calculate_padded_page_count(12, PageArrangement::Octavo), 16);

    // 17 pages padded to octavo = 32
    assert_eq!(calculate_padded_page_count(17, PageArrangement::Octavo), 32);
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
