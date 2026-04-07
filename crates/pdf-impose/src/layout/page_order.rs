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
/// Returns one `SignaturePageAssignment` per sheet in the signature.
/// For single-sheet signatures (sheets_per_signature=1), the result
/// has exactly one element matching the previous behavior.
///
/// # Arguments
/// * `arrangement` - The page arrangement (folio, quarto, octavo)
/// * `sheets_per_signature` - Number of sheets nested together
/// * `sig_start` - First page index for this signature (0-based)
/// * `total_source_pages` - Total number of source pages available
pub fn assign_pages_to_spreads(
    arrangement: PageArrangement,
    sheets_per_signature: usize,
    sig_start: usize,
    total_source_pages: usize,
) -> Vec<SignaturePageAssignment> {
    let (front_order, back_order) = page_order_for_arrangement(arrangement);
    let pages_per_sheet = arrangement.pages_per_sheet();

    (0..sheets_per_signature)
        .map(|sheet_idx| {
            let remap = build_nesting_remap(sheet_idx, sheets_per_signature, pages_per_sheet);

            let make_spreads = |order: &[usize]| -> Vec<Spread> {
                order
                    .chunks(2)
                    .map(|chunk| {
                        let verso_abs = sig_start + remap[chunk[0]];
                        let recto_abs = sig_start + remap[chunk[1]];
                        let verso = Some(verso_abs).filter(|&idx| idx < total_source_pages);
                        let recto = Some(recto_abs).filter(|&idx| idx < total_source_pages);
                        Spread::new(verso, recto)
                    })
                    .collect()
            };

            SignaturePageAssignment {
                front: make_spreads(&front_order),
                back: make_spreads(&back_order),
            }
        })
        .collect()
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
            6, 9, // Top-left (was top-right): [verso=7, recto=10]
            10, 5, // Top-right (was top-left): [verso=11, recto=6]
        ],
    )
}

/// Build a page index remap for saddle-stitch nesting.
///
/// When multiple sheets are nested together in a signature, the outermost
/// sheet carries the first and last leaves, the next sheet carries the
/// next-outermost leaves, and so on.
///
/// For sheet `i` (0 = outermost) in an S-sheet signature with P pages per sheet:
/// - First half of single-sheet indices (0..P/2) map to: `i*(P/2) + j`
/// - Second half (P/2..P) map to: `S*P - (i+1)*(P/2) + (j - P/2)`
///
/// For a single-sheet signature (S=1), this is the identity mapping.
fn build_nesting_remap(
    sheet_index: usize,
    sheets: usize,
    pages_per_sheet: usize,
) -> Vec<usize> {
    let half = pages_per_sheet / 2;
    let total = sheets * pages_per_sheet;

    (0..pages_per_sheet)
        .map(|j| {
            if j < half {
                sheet_index * half + j
            } else {
                total - (sheet_index + 1) * half + (j - half)
            }
        })
        .collect()
}

// =============================================================================
// Signature Calculation
// =============================================================================

/// Calculate the number of signatures needed for a given page count.
pub fn calculate_signature_count(total_pages: usize, pages_per_signature: usize) -> usize {
    (total_pages + pages_per_signature - 1) / pages_per_signature
}

/// Calculate total pages with padding to fill complete signatures.
pub fn calculate_padded_page_count(total_pages: usize, pages_per_signature: usize) -> usize {
    let signatures = calculate_signature_count(total_pages, pages_per_signature);
    signatures * pages_per_signature
}

#[cfg(test)]
#[path = "tests/page_order_tests.rs"]
mod tests;
