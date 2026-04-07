use lopdf::Document;
use pdf_async_runtime::{ImpositionOptions, PdfUpdate};
use pdf_impose::{calculate_statistics, generate_preview, impose, load_multiple_pdfs, save_pdf};
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Caches loaded source documents to avoid reloading on every preview
pub struct ImposeDocStore {
    source_cache: Option<SourceDocCache>,
}

struct SourceDocCache {
    paths: Vec<PathBuf>,
    documents: Vec<Document>,
}

impl ImposeDocStore {
    pub fn new() -> Self {
        Self {
            source_cache: None,
        }
    }

    /// Get cached source documents if the paths match, otherwise load and cache
    pub async fn get_or_load_sources(
        &mut self,
        paths: &[PathBuf],
    ) -> Result<&[Document], pdf_impose::ImposeError> {
        let cache_valid = self
            .source_cache
            .as_ref()
            .map(|c| c.paths == paths)
            .unwrap_or(false);

        if !cache_valid {
            log::debug!("Loading source documents (cache miss or paths changed)");
            let documents = load_multiple_pdfs(paths).await?;
            self.source_cache = Some(SourceDocCache {
                paths: paths.to_vec(),
                documents,
            });
        } else {
            log::debug!("Using cached source documents");
        }

        Ok(&self.source_cache.as_ref().unwrap().documents)
    }
}

pub async fn handle_load(input_path: PathBuf, update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    match pdf_impose::load_pdf(&input_path).await {
        Ok(doc) => {
            let page_count = doc.get_pages().len();
            let _ = update_tx.send(PdfUpdate::ImposeLoaded {
                doc_id: pdf_async_runtime::DocumentId(0),
                page_count,
            });
        }
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDF: {e}"),
            });
        }
    }
}

pub async fn handle_process(update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    let _ = update_tx.send(PdfUpdate::Error {
        message: "Imposition not yet fully implemented".to_string(),
    });
}

pub async fn handle_generate_preview(
    options: ImpositionOptions,
    doc_store: &mut ImposeDocStore,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    if options.input_files.is_empty() {
        let _ = update_tx.send(PdfUpdate::Error {
            message: "No input files specified".to_string(),
        });
        return;
    }

    // Get cached documents or load them (avoids reloading on every preview)
    let paths: Vec<PathBuf> = options.input_files.iter().cloned().collect();
    let documents = match doc_store.get_or_load_sources(&paths).await {
        Ok(docs) => docs,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDFs: {}", e),
            });
            return;
        }
    };

    // Calculate and send statistics
    if let Ok(stats) = calculate_statistics(documents, &options) {
        let _ = update_tx.send(PdfUpdate::ImposeStatsCalculated { stats });
    }

    // Generate preview (first signature or reasonable sample)
    let mut preview = match generate_preview(documents, &options, 4).await {
        Ok(doc) => doc,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to generate preview: {}", e),
            });
            return;
        }
    };

    let page_count = preview.get_pages().len();

    // Serialize to bytes for in-memory viewer loading (no disk round-trip)
    let mut pdf_bytes = Vec::new();
    if let Err(e) = preview.save_to(&mut pdf_bytes) {
        let _ = update_tx.send(PdfUpdate::Error {
            message: format!("Failed to serialize preview: {}", e),
        });
        return;
    }

    let _ = update_tx.send(PdfUpdate::ImposePreviewGenerated {
        pdf_bytes,
        page_count,
    });
}

pub async fn handle_generate(
    options: ImpositionOptions,
    output_path: PathBuf,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    if options.input_files.is_empty() {
        let _ = update_tx.send(PdfUpdate::Error {
            message: "No input files specified".to_string(),
        });
        return;
    }

    let _ = update_tx.send(PdfUpdate::Progress {
        operation: "Loading PDFs".to_string(),
        current: 0,
        total: options.input_files.len(),
    });

    // Load documents
    let paths: Vec<PathBuf> = options.input_files.iter().cloned().collect();
    let documents = match load_multiple_pdfs(&paths).await {
        Ok(docs) => docs,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDFs: {}", e),
            });
            return;
        }
    };

    let _ = update_tx.send(PdfUpdate::Progress {
        operation: "Imposing pages".to_string(),
        current: 1,
        total: 3,
    });

    // Impose
    let imposed = match impose(&documents, &options).await {
        Ok(doc) => doc,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to impose PDF: {}", e),
            });
            return;
        }
    };

    let _ = update_tx.send(PdfUpdate::Progress {
        operation: "Saving PDF".to_string(),
        current: 2,
        total: 3,
    });

    // Save
    if let Err(e) = save_pdf(imposed, &output_path).await {
        let _ = update_tx.send(PdfUpdate::Error {
            message: format!("Failed to save PDF: {}", e),
        });
        return;
    }

    let _ = update_tx.send(PdfUpdate::ImposeComplete { path: output_path });
}

pub async fn handle_load_config(path: PathBuf, update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    match ImpositionOptions::load(&path).await {
        Ok(options) => {
            let _ = update_tx.send(PdfUpdate::ImposeConfigLoaded { options });
        }
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load configuration: {}", e),
            });
        }
    }
}

pub async fn handle_calculate_stats(
    options: ImpositionOptions,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    if options.input_files.is_empty() {
        return;
    }

    // Load documents
    let paths: Vec<PathBuf> = options.input_files.iter().cloned().collect();
    let documents = match load_multiple_pdfs(&paths).await {
        Ok(docs) => docs,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDFs for stats: {}", e),
            });
            return;
        }
    };

    // Calculate statistics
    match calculate_statistics(&documents, &options) {
        Ok(stats) => {
            let _ = update_tx.send(PdfUpdate::ImposeStatsCalculated { stats });
        }
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to calculate statistics: {}", e),
            });
        }
    }
}
