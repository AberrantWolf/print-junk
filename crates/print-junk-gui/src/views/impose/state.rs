use pdf_impose::{ImpositionOptions, ImpositionStatistics};
use std::path::PathBuf;

use super::super::ViewerState;

#[derive(Default)]
pub struct ImposeState {
    pub options: ImpositionOptions,
    pub preview_page_count: usize,
    pub stats: Option<ImpositionStatistics>,
    #[allow(dead_code)]
    pub loaded_docs: Vec<(PathBuf, usize)>,
    pub preview_viewer: Option<ViewerState>,
    pub needs_regeneration: bool,
    /// Number of signatures shown in the current preview (None if no preview)
    pub preview_signatures_shown: Option<usize>,
    /// Total signatures in the full imposition (None if no preview)
    pub preview_total_signatures: Option<usize>,
}
