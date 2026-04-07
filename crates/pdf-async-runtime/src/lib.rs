use std::path::PathBuf;

// Re-export types from library crates
pub use pdf_flashcards::{Flashcard, FlashcardOptions};
pub use pdf_impose::{ImpositionOptions, ImpositionStatistics};

/// Commands sent from UI to worker
#[derive(Debug)]
pub enum PdfCommand {
    FlashcardsLoadCsv {
        input_path: PathBuf,
    },
    FlashcardsGenerate {
        cards: Vec<Flashcard>,
        options: FlashcardOptions,
        output_path: PathBuf,
    },
    ImposeLoad {
        input_path: PathBuf,
    },
    ImposeProcess {
        doc_id: DocumentId,
        options: ImpositionOptions,
        output_path: PathBuf,
    },
    ImposeGeneratePreview {
        options: ImpositionOptions,
    },
    ImposeGenerate {
        options: ImpositionOptions,
        output_path: PathBuf,
    },
    ImposeLoadConfig {
        path: PathBuf,
    },
    ImposeCalculateStats {
        options: ImpositionOptions,
    },
    ViewerLoad {
        path: PathBuf,
    },
    ViewerLoadBytes {
        pdf_bytes: Vec<u8>,
        page_count: usize,
    },
    ViewerRenderPage {
        doc_id: DocumentId,
        page_index: usize,
        /// Zoom level as a fraction (1.0 = 100%). Use 0.0 for legacy fixed-size rendering.
        zoom_level: f32,
    },
    /// Prefetch pages for faster navigation (lower priority than direct renders)
    ViewerPrefetchPages {
        doc_id: DocumentId,
        page_indices: Vec<usize>,
        /// Zoom level as a fraction (1.0 = 100%). Use 0.0 for legacy fixed-size rendering.
        zoom_level: f32,
    },
    ViewerClose {
        doc_id: DocumentId,
    },
}

/// Updates sent from worker to UI
#[derive(Debug, Clone)]
pub enum PdfUpdate {
    Progress {
        operation: String,
        current: usize,
        total: usize,
    },
    FlashcardsLoaded {
        cards: Vec<Flashcard>,
    },
    FlashcardsComplete {
        path: PathBuf,
        card_count: usize,
    },
    ImposeLoaded {
        doc_id: DocumentId,
        page_count: usize,
    },
    ImposeComplete {
        path: PathBuf,
    },
    ImposePreviewGenerated {
        pdf_bytes: Vec<u8>,
        page_count: usize,
        /// Number of signatures included in this preview
        signatures_shown: usize,
        /// Total signatures in the full imposition
        total_signatures: usize,
    },
    ImposeConfigLoaded {
        options: ImpositionOptions,
    },
    ImposeStatsCalculated {
        stats: ImpositionStatistics,
    },
    Error {
        message: String,
    },
    ViewerLoaded {
        doc_id: DocumentId,
        page_count: usize,
    },
    ViewerPageRendered {
        doc_id: DocumentId,
        page_index: usize,
        width: usize,
        height: usize,
        rgba_data: Vec<u8>,
        /// The zoom level this was rendered at (fraction, 1.0 = 100%)
        zoom_level: f32,
        /// Native page width in PDF points
        page_width_pts: f32,
        /// Native page height in PDF points
        page_height_pts: f32,
    },
    ViewerClosed {
        doc_id: DocumentId,
    },
}

/// Handle to a loaded document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(pub u64);
