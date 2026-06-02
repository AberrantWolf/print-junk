use lopdf::Document;
use pdf_async_runtime::{ImpositionOptions, PdfUpdate};
use pdf_impose::{calculate_statistics, generate_preview, impose_and_save, load_multiple_pdfs};
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
        Self { source_cache: None }
    }

    /// Get cached source documents if the paths match, otherwise load and cache
    pub async fn get_or_load_sources(
        &mut self,
        paths: &[PathBuf],
    ) -> Result<&[Document], pdf_impose::ImposeError> {
        let cache_valid = self.source_cache.as_ref().is_some_and(|c| c.paths == paths);

        if cache_valid {
            log::debug!("Using cached source documents");
        } else {
            log::debug!("Loading source documents (cache miss or paths changed)");
            let documents = load_multiple_pdfs(paths).await?;
            self.source_cache = Some(SourceDocCache {
                paths: paths.to_vec(),
                documents,
            });
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

pub fn handle_process(update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
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
    let paths: Vec<PathBuf> = options.input_files.clone();
    let documents = match doc_store.get_or_load_sources(&paths).await {
        Ok(docs) => docs,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDFs: {e}"),
            });
            return;
        }
    };

    // Calculate and send statistics (also used for total_signatures below)
    let stats = match calculate_statistics(documents, &options) {
        Ok(stats) => {
            let _ = update_tx.send(PdfUpdate::ImposeStatsCalculated {
                stats: stats.clone(),
            });
            Some(stats)
        }
        Err(_) => None,
    };
    let total_signatures = stats.and_then(|s| s.signatures).unwrap_or(0);

    // Generate preview (smart default limits output to ~16 sheets)
    let preview_docs = documents.to_vec();
    let mut preview_result = match generate_preview(preview_docs, &options, None).await {
        Ok(r) => r,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to generate preview: {e}"),
            });
            return;
        }
    };

    let page_count = preview_result.document.get_pages().len();
    let signatures_shown = preview_result.signatures_shown;

    // Serialize to bytes for in-memory viewer loading (no disk round-trip)
    let mut pdf_bytes = Vec::new();
    if let Err(e) = preview_result.document.save_to(&mut pdf_bytes) {
        let _ = update_tx.send(PdfUpdate::Error {
            message: format!("Failed to serialize preview: {e}"),
        });
        return;
    }

    let _ = update_tx.send(PdfUpdate::ImposePreviewGenerated {
        pdf_bytes,
        page_count,
        signatures_shown,
        total_signatures,
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
    let paths: Vec<PathBuf> = options.input_files.clone();
    let documents = match load_multiple_pdfs(&paths).await {
        Ok(docs) => docs,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDFs: {e}"),
            });
            return;
        }
    };

    let _ = update_tx.send(PdfUpdate::Progress {
        operation: "Imposing and saving".to_string(),
        current: 1,
        total: 2,
    });

    // Impose and save (handles split_mode internally)
    let saved_paths = match impose_and_save(documents, &options, &output_path).await {
        Ok(paths) => paths,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to impose PDF: {e}"),
            });
            return;
        }
    };

    let primary_path = saved_paths
        .first()
        .cloned()
        .unwrap_or_else(|| output_path.clone());

    if saved_paths.len() > 1 {
        log::info!("Saved {} signature-split PDFs:", saved_paths.len());
        for p in &saved_paths {
            log::info!("  {}", p.display());
        }
    }

    let _ = update_tx.send(PdfUpdate::ImposeComplete { path: primary_path });
}

pub async fn handle_load_config(path: PathBuf, update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    match ImpositionOptions::load(&path).await {
        Ok(options) => {
            let _ = update_tx.send(PdfUpdate::ImposeConfigLoaded { options });
        }
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load configuration: {e}"),
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
    let paths: Vec<PathBuf> = options.input_files.clone();
    let documents = match load_multiple_pdfs(&paths).await {
        Ok(docs) => docs,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDFs for stats: {e}"),
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
                message: format!("Failed to calculate statistics: {e}"),
            });
        }
    }
}
