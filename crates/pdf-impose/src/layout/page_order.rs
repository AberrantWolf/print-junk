//! Page ordering within signatures
//!
//! This module assigns page numbers to spread positions based on
//! traditional bookbinding rules. When you fold a sheet of paper,
//! the pages appear in a specific order - this module calculates
//! that order.
//!
//! ## Bookbinding Page Order
//!
//! For a **folio** (4 pages, 1 fold):
//! - Side A (front): verso=4, recto=1
//! - Side B (back): verso=2, recto=3
//!
//! For a **quarto** (8 pages, 2 folds):
//! - Side A bottom: verso=8, recto=1
//! - Side A top (rotated): verso=5, recto=4
//! - Side B bottom: verso=2, recto=7
//! - Side B top (rotated): verso=3, recto=6
//!
//! For an **octavo** (16 pages, 3 folds):
//! - 4 spreads per side, with top row rotated 180 degrees
//!
//! ## Key Insight
//!
//! The page ordering follows from how paper folds:
//! - Pages at the "outside" of the folded sheet have the highest/lowest numbers
//! - Pages toward the "inside" have middle numbers
//! - The top row is rotated because it folds over

use crate::types::PageArrangement;

use super::{SheetSide, Spread, SpreadPosition};

// =============================================================================
// Page Assignment
// =============================================================================

/// Page assignments for both sides of a signature sheet
#[derive(Debug, Clone)]
pub struct SignaturePageAssignment {
    /// Spreads for the front side of the sheet
    pub front: Vec<Spread>,
    /// Spreads for the back side of the sheet
    pub back: Vec<Spread>,
}

impl SignaturePageAssignment {
    /// Get spreads for a specific sheet side
    pub fn for_side(&self, side: SheetSide) -> &[Spread] {
        match side {
            SheetSide::Front => &self.front,
            SheetSide::Back => &self.back,
        }
    }

    /// Total number of pages in this assignment (including blanks counted as 0)
    pub fn page_count(&self) -> usize {
        let count_pages = |spreads: &[Spread]| {
            spreads
                .iter()
                .map(|s| {
                    (if s.verso_page.is_some() { 1 } else { 0 })
                        + (if s.recto_page.is_some() { 1 } else { 0 })
                })
                .sum::<usize>()
        };
        count_pages(&self.front) + count_pages(&self.back)
    }
}

/// Assign pages to spreads for a signature.
///
/// # Arguments
/// * `arrangement` - The page arrangement (folio, quarto, octavo)
/// * `sig_start` - First page index for this signature (0-based)
/// * `total_source_pages` - Total number of source pages available
///
/// # Returns
/// Page assignments for front and back sides of the sheet.
pub fn assign_pages_to_spreads(
    arrangement: PageArrangement,
    sig_start: usize,
    total_source_pages: usize,
) -> SignaturePageAssignment {
    // Get the raw page order (0-indexed relative to signature start)
    let (front_order, back_order) = page_order_for_arrangement(arrangement);

    // Convert relative indices to absolute page indices (or None for blanks)
    let to_spread = |verso_rel: usize, recto_rel: usize| -> Spread {
        let verso = sig_start
            .checked_add(verso_rel)
            .filter(|&idx| idx < total_source_pages);
        let recto = sig_start
            .checked_add(recto_rel)
            .filter(|&idx| idx < total_source_pages);
        Spread::new(verso, recto)
    };

    let front: Vec<Spread> = front_order
        .chunks(2)
        .map(|chunk| to_spread(chunk[0], chunk[1]))
        .collect();

    let back: Vec<Spread> = back_order
        .chunks(2)
        .map(|chunk| to_spread(chunk[0], chunk[1]))
        .collect();

    SignaturePageAssignment { front, back }
}

/// Apply page assignments to spread positions.
///
/// Takes spread positions (geometry) and page assignments (content)
/// and combines them into fully-specified spread positions.
pub fn apply_page_assignments(
    positions: &[SpreadPosition],
    assignments: &[Spread],
) -> Vec<SpreadPosition> {
    positions
        .iter()
        .zip(assignments.iter())
        .map(|(pos, spread)| SpreadPosition {
            spread: spread.clone(),
            origin: pos.origin,
            width: pos.width,
            height: pos.height,
            rotated: pos.rotated,
            spread_index: pos.spread_index,
        })
        .collect()
}

// =============================================================================
// Page Order Tables
// =============================================================================

/// Get the page order for an arrangement.
///
/// Returns (front_order, back_order) where each is a flat array of
/// 0-indexed page numbers in [verso, recto, verso, recto, ...] order.
fn page_order_for_arrangement(arrangement: PageArrangement) -> (Vec<usize>, Vec<usize>) {
    match arrangement {
        PageArrangement::Folio => folio_page_order(),
        PageArrangement::Quarto => quarto_page_order(),
        PageArrangement::Octavo => octavo_page_order(),
        PageArrangement::Custom {
            pages_per_signature,
        } => custom_page_order(pages_per_signature),
    }
}

/// Folio page order (4 pages, 1 spread per side)
///
/// After folding once:
/// - Outside (Side A): pages 4, 1
/// - Inside (Side B): pages 2, 3
fn folio_page_order() -> (Vec<usize>, Vec<usize>) {
    (
        vec![3, 0], // Front: [verso=4, recto=1] (0-indexed: 3, 0)
        vec![1, 2], // Back: [verso=2, recto=3] (0-indexed: 1, 2)
    )
}

/// Quarto page order (8 pages, 2 spreads per side)
///
/// Spread order: [bottom, top] where top is rotated 180 degrees
///
/// Side A:
/// - Bottom spread: verso=8, recto=1
/// - Top spread (rotated): verso=5, recto=4
///
/// Side B:
/// - Bottom spread: verso=2, recto=7
/// - Top spread (rotated): verso=3, recto=6
fn quarto_page_order() -> (Vec<usize>, Vec<usize>) {
    (
        vec![
            7, 0, // Bottom: [verso=8, recto=1]
            4, 3, // Top: [verso=5, recto=4]
        ],
        vec![
            1, 6, // Bottom: [verso=2, recto=7]
            2, 5, // Top: [verso=3, recto=6]
        ],
    )
}

/// Octavo page order (16 pages, 4 spreads per side)
///
/// Spread order: [bottom-left, bottom-right, top-left, top-right]
/// Top row is rotated 180 degrees.
///
/// The page order follows traditional bookbinding conventions.
fn octavo_page_order() -> (Vec<usize>, Vec<usize>) {
    // From the diagram, spread positions are [bottom-left, bottom-right, top-left, top-right]
    // Side B is the flipped sheet, so spread positions mirror horizontally
    (
        vec![
            // Side A - Bottom row (not rotated)
            3, 12, // Bottom-left: [verso=4, recto=13]
            15, 0, // Bottom-right: [verso=16, recto=1]
            // Side A - Top row (rotated 180 degrees)
            4, 11, // Top-left: [verso=5, recto=12]
            8, 7, // Top-right: [verso=9, recto=8]
        ],
        vec![
            // Side B - Bottom row (not rotated)
            // When sheet flips, left becomes right, so order reverses
            1, 14, // Bottom-left (was bottom-right): [verso=2, recto=15]
            13, 2, // Bottom-right (was bottom-left): [verso=14, recto=3]
            // Side B - Top row (rotated 180 degrees)
            5, 10, // Top-left (was top-right): [verso=6, recto=11]
            9, 6, // Top-right (was top-left): [verso=10, recto=7]
        ],
    )
}

/// Custom page order using saddle-stitch pattern.
///
/// For custom page counts, we use a simple saddle-stitch pattern
/// where pages are paired from outside to inside.
fn custom_page_order(pages_per_signature: usize) -> (Vec<usize>, Vec<usize>) {
    // For simplicity, treat as multiple folios
    let sheets = pages_per_signature / 4;
    let mut front = Vec::with_capacity(sheets * 2);
    let mut back = Vec::with_capacity(sheets * 2);

    for i in 0..sheets {
        let last = pages_per_signature - 1 - (2 * i);
        let first = 2 * i;

        // Front: [last, first]
        front.push(last);
        front.push(first);

        // Back: [first+1, last-1]
        back.push(first + 1);
        back.push(last - 1);
    }

    (front, back)
}

// =============================================================================
// Signature Calculation
// =============================================================================

/// Calculate the number of signatures needed for a given page count.
pub fn calculate_signature_count(total_pages: usize, arrangement: PageArrangement) -> usize {
    let pages_per_sig = arrangement.pages_per_signature();
    (total_pages + pages_per_sig - 1) / pages_per_sig
}

/// Calculate total pages with padding to fill complete signatures.
pub fn calculate_padded_page_count(total_pages: usize, arrangement: PageArrangement) -> usize {
    let pages_per_sig = arrangement.pages_per_signature();
    let signatures = calculate_signature_count(total_pages, arrangement);
    signatures * pages_per_sig
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
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
}
