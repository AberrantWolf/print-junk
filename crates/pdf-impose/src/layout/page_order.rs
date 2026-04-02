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

#[cfg(test)]
#[path = "tests/page_order_tests.rs"]
mod tests;
