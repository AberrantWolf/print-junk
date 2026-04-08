//! Statistics calculation for imposition
//!
//! Calculates output statistics without performing the actual imposition.

use crate::constants::PAGES_PER_LEAF;
use crate::options::ImpositionOptions;
use crate::types::{ImposeError, ImpositionStatistics, Result, Warning};
use lopdf::Document;

/// Calculate statistics for the imposition
///
/// Returns statistics about the output without generating the actual PDF.
pub fn calculate_statistics(
    documents: &[Document],
    options: &ImpositionOptions,
) -> Result<ImpositionStatistics> {
    // Count total source pages
    let mut source_pages: usize = documents.iter().map(|doc| doc.get_pages().len()).sum();

    // Add flyleaves (each flyleaf = 1 leaf = 2 pages)
    source_pages += (options.front_flyleaves + options.back_flyleaves) * PAGES_PER_LEAF;

    if source_pages == 0 {
        return Err(ImposeError::NoPages);
    }

    log::debug!(
        "Calculating stats: {} source pages, {:?} binding, {:?} arrangement",
        source_pages,
        options.binding_type,
        options.page_arrangement
    );

    if options.binding_type.uses_signatures() {
        Ok(calculate_signature_stats(source_pages, options))
    } else {
        Ok(calculate_simple_stats(source_pages, options))
    }
}

/// Calculate statistics for signature binding
fn calculate_signature_stats(
    source_pages: usize,
    options: &ImpositionOptions,
) -> ImpositionStatistics {
    let pages_per_sig = options.pages_per_signature();
    let sheets_per_sig = options.sheets_per_signature;

    // Pad to multiple of pages_per_signature
    let padded_count = round_up_to_multiple(source_pages, pages_per_sig);
    let blank_pages_added = padded_count - source_pages;

    let num_signatures = padded_count / pages_per_sig;
    let total_sheets = num_signatures * sheets_per_sig;

    let warnings = blank_padding_warnings(blank_pages_added, padded_count);

    let cascade_cells_per_sheet = options
        .cascade
        .as_ref()
        .filter(|c| !c.is_trivial())
        .map(|c| c.cells());

    let (output_sheets, output_pages) = if let Some(cells) = cascade_cells_per_sheet {
        let cascade_sheets = total_sheets.div_ceil(cells);
        (cascade_sheets, cascade_sheets * 2)
    } else {
        (total_sheets, total_sheets * 2)
    };

    ImpositionStatistics {
        source_pages,
        output_sheets,
        signatures: Some(num_signatures),
        pages_per_signature: Some(vec![pages_per_sig; num_signatures]),
        output_pages,
        blank_pages_added,
        cascade_cells_per_sheet,
        warnings,
    }
}

/// Calculate statistics for simple 2-up binding
fn calculate_simple_stats(
    source_pages: usize,
    options: &ImpositionOptions,
) -> ImpositionStatistics {
    // Perfect binding, side stitch, spiral: 2 pages per sheet
    let padded_count = round_up_to_multiple(source_pages, 2);
    let blank_pages_added = padded_count - source_pages;

    let total_sheets = padded_count / 2;

    let cascade_cells_per_sheet = options
        .cascade
        .as_ref()
        .filter(|c| !c.is_trivial())
        .map(|c| c.cells());

    let (output_sheets, output_pages) = if let Some(cells) = cascade_cells_per_sheet {
        let cascade_sheets = total_sheets.div_ceil(cells);
        (cascade_sheets, cascade_sheets * 2)
    } else {
        (total_sheets, total_sheets * 2)
    };

    let warnings = blank_padding_warnings(blank_pages_added, padded_count);

    ImpositionStatistics {
        source_pages,
        output_sheets,
        signatures: None,
        pages_per_signature: None,
        output_pages,
        blank_pages_added,
        cascade_cells_per_sheet,
        warnings,
    }
}

/// Generate warnings if blank page padding exceeds 25% of total capacity
fn blank_padding_warnings(blank_pages_added: usize, padded_count: usize) -> Vec<Warning> {
    let mut warnings = Vec::new();
    if padded_count > 0 && blank_pages_added > 0 {
        let percent = blank_pages_added as f32 / padded_count as f32 * 100.0;
        if percent > 25.0 {
            warnings.push(Warning::ExcessiveBlankPadding {
                blank_count: blank_pages_added,
                total_pages: padded_count,
                percent,
            });
        }
    }
    warnings
}

/// Round up to the nearest multiple
fn round_up_to_multiple(value: usize, multiple: usize) -> usize {
    value.div_ceil(multiple) * multiple
}
