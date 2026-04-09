//! Preview generation for imposition
//!
//! Generates a limited preview of the imposition for quick display.

use crate::impose::{PageSource, impose_page_source};
use crate::options::ImpositionOptions;
use crate::types::Result;
use lopdf::Document;

/// Result of preview generation, including truncation metadata.
pub struct PreviewResult {
    /// The imposed preview document
    pub document: Document,
    /// How many signatures are represented in the preview
    pub signatures_shown: usize,
}

/// Maximum output sheets to render in a preview.
///
/// The actual signature limit is derived from this based on `sheets_per_signature`,
/// so simpler arrangements (folio) show more signatures than complex ones (octavo).
const MAX_PREVIEW_SHEETS: usize = 16;

/// Generate a preview of the imposition
///
/// Returns an imposed document limited to `max_signatures` complete signatures.
/// If `None`, a smart default is computed targeting ~[`MAX_PREVIEW_SHEETS`] output sheets.
pub async fn generate_preview(
    documents: Vec<Document>,
    options: &ImpositionOptions,
    max_signatures: Option<usize>,
) -> Result<PreviewResult> {
    let total_source_pages: usize = documents.iter().map(|d| d.get_pages().len()).sum();

    let (source_pages_needed, signatures_shown) = if options.binding_type.uses_signatures() {
        let pages_per_sig = options.pages_per_signature();
        let effective_max = max_signatures
            .unwrap_or_else(|| (MAX_PREVIEW_SHEETS / options.sheets_per_signature).max(1));
        let total_sigs = total_source_pages.div_ceil(pages_per_sig);
        let sigs = total_sigs.min(effective_max);
        (sigs * pages_per_sig, sigs)
    } else {
        let pages_needed = max_signatures.unwrap_or(MAX_PREVIEW_SHEETS) * 2;
        let sigs = pages_needed.min(total_source_pages);
        (sigs, 0)
    };

    // Build a PageSource with limited pages — no deep copy needed
    let page_source = PageSource::with_page_limit(
        documents,
        options.front_flyleaves,
        options.back_flyleaves,
        source_pages_needed,
    )?;

    let options = options.clone();
    let document = tokio::task::spawn_blocking(move || {
        options.validate()?;
        impose_page_source(&page_source, &options)
    })
    .await??;

    Ok(PreviewResult {
        document,
        signatures_shown,
    })
}
