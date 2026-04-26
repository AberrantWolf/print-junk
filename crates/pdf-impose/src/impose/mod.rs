//! PDF imposition - arranging pages for bookbinding
//!
//! This module orchestrates the imposition process:
//! 1. Build a `PageSource` from input documents (with optional flyleaves)
//! 2. Calculate signature layouts
//! 3. Place pages with margins and alignment
//! 4. Render to output PDF with printer's marks

mod cascade;
mod io;
mod page_source;
mod sheet;
mod signature;
mod simple;

pub use io::{impose_and_save, load_multiple_pdfs, load_pdf, save_pdf};
pub use page_source::{PageSource, XObjectCache};

use crate::options::ImpositionOptions;
use crate::types::Result;
use lopdf::Document;

// =============================================================================
// Main Entry Point
// =============================================================================

/// Main imposition function
///
/// Takes ownership of source documents and options, returns an imposed output document.
pub async fn impose(documents: Vec<Document>, options: &ImpositionOptions) -> Result<Document> {
    options.validate()?;

    let options = options.clone();

    tokio::task::spawn_blocking(move || impose_sync(documents, &options)).await?
}

fn impose_sync(documents: Vec<Document>, options: &ImpositionOptions) -> Result<Document> {
    let page_source = PageSource::new(documents, options.front_flyleaves, options.back_flyleaves)?;
    impose_page_source(&page_source, options)
}

/// Impose from a pre-built `PageSource`. Used by preview to apply page limits.
pub(crate) fn impose_page_source(
    page_source: &PageSource,
    options: &ImpositionOptions,
) -> Result<Document> {
    let total_pages = page_source.len();

    if total_pages == 0 {
        return Err(crate::types::ImposeError::NoPages);
    }

    log::info!(
        "Imposing {} pages: {:?} binding, {:?} arrangement, {} sheets/sig",
        total_pages,
        options.binding_type,
        options.page_arrangement,
        options.sheets_per_signature
    );

    // Dispatch based on binding type
    if options.binding_type.uses_signatures() {
        signature::impose_signature_binding(page_source, options)
    } else {
        simple::impose_simple_binding(page_source, options)
    }
}
