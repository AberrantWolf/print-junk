use pdf_async_runtime::DocumentId;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "pdf-viewer")]
use pdfium_render::prelude::*;

/// Initialize Pdfium, trying the vendored library first, then falling back to system
#[cfg(feature = "pdf-viewer")]
pub fn init_pdfium() -> Result<Pdfium, PdfiumError> {
    // Try to load from vendor directory (relative to workspace root)
    // When running from cargo, the working directory is the workspace root
    let vendor_path = std::env::current_dir().ok().and_then(|mut p| {
        p.push("vendor/pdfium/lib");
        if p.exists() { Some(p) } else { None }
    });

    if let Some(vendor_path) = vendor_path
        && let Ok(binding) =
            Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&vendor_path))
    {
        return Ok(Pdfium::new(binding));
    }

    // Fallback to system library or default search paths
    Pdfium::bind_to_system_library().map(Pdfium::new)
}

/// Cached page data
#[cfg(feature = "pdf-viewer")]
pub struct CachedPage {
    pub rgba_data: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

/// Cache key: (document, page index, quantized zoom percentage)
#[cfg(feature = "pdf-viewer")]
type CacheKey = (DocumentId, usize, u32);

/// Maximum number of pages to cache
#[cfg(feature = "pdf-viewer")]
const MAX_CACHED_PAGES: usize = 50;

/// Maximum render dimension in pixels (to prevent OOM at extreme zoom)
#[cfg(feature = "pdf-viewer")]
const MAX_RENDER_DIMENSION: i32 = 4096;

/// Legacy render dimensions used when `zoom_level` is 0.0 (impose/flashcard previews)
#[cfg(feature = "pdf-viewer")]
const LEGACY_TARGET_WIDTH: i32 = 600;
#[cfg(feature = "pdf-viewer")]
const LEGACY_MAX_HEIGHT: i32 = 800;

/// Quantize a zoom fraction to a discrete percentage for cache keys.
/// Steps: every 25% from 25-100, every 50% from 100-400.
pub fn quantize_zoom(zoom: f32) -> u32 {
    let percent = (zoom * 100.0).round() as i32;
    let clamped = percent.clamp(25, 400);
    if clamped <= 100 {
        // Round to nearest 25
        ((clamped + 12) / 25 * 25) as u32
    } else {
        // Round to nearest 50
        ((clamped + 25) / 50 * 50) as u32
    }
}

/// Compute render dimensions for a given page size and zoom level.
/// Returns (`target_width`, `max_height`) suitable for `PdfRenderConfig`.
/// If zoom is 0.0, returns legacy fixed dimensions (600x800).
#[cfg(feature = "pdf-viewer")]
pub fn render_dimensions(page_width_pts: f32, page_height_pts: f32, zoom: f32) -> (i32, i32) {
    if zoom <= 0.0 {
        return (LEGACY_TARGET_WIDTH, LEGACY_MAX_HEIGHT);
    }
    let w = (page_width_pts * zoom).round() as i32;
    let h = (page_height_pts * zoom).round() as i32;
    (w.min(MAX_RENDER_DIMENSION), h.min(MAX_RENDER_DIMENSION))
}

/// Build a `PdfRenderConfig` for the given dimensions.
#[cfg(feature = "pdf-viewer")]
pub fn make_render_config(page_width_pts: f32, page_height_pts: f32, zoom: f32) -> PdfRenderConfig {
    let (w, h) = render_dimensions(page_width_pts, page_height_pts, zoom);
    PdfRenderConfig::new()
        .set_target_width(w)
        .set_maximum_height(h)
}

/// A document source: either a file path or in-memory PDF bytes
#[cfg(feature = "pdf-viewer")]
#[derive(Clone)]
pub enum DocumentSource {
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// State for PDF viewer functionality
#[cfg(feature = "pdf-viewer")]
pub struct ViewerState {
    documents: HashMap<DocumentId, DocumentSource>,
    page_cache: HashMap<CacheKey, CachedPage>,
    cache_order: VecDeque<CacheKey>,
    next_doc_id: AtomicU64,
}

#[cfg(feature = "pdf-viewer")]
impl ViewerState {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            page_cache: HashMap::new(),
            cache_order: VecDeque::new(),
            next_doc_id: AtomicU64::new(0),
        }
    }

    pub fn next_id(&self) -> DocumentId {
        DocumentId(self.next_doc_id.fetch_add(1, Ordering::SeqCst))
    }

    pub fn add_document(&mut self, doc_id: DocumentId, path: PathBuf) {
        self.documents.insert(doc_id, DocumentSource::File(path));
    }

    pub fn add_document_bytes(&mut self, doc_id: DocumentId, bytes: Vec<u8>) {
        self.documents.insert(doc_id, DocumentSource::Bytes(bytes));
    }

    pub fn get_document(&self, doc_id: DocumentId) -> Option<&DocumentSource> {
        self.documents.get(&doc_id)
    }

    pub fn add_to_cache(&mut self, key: CacheKey, page: CachedPage) {
        // Remove if already exists (update LRU)
        if self.page_cache.contains_key(&key) {
            self.cache_order.retain(|k| k != &key);
        }

        // Evict LRU if full
        while self.cache_order.len() >= MAX_CACHED_PAGES {
            if let Some(old_key) = self.cache_order.pop_front() {
                self.page_cache.remove(&old_key);
            }
        }

        // Add to cache
        self.page_cache.insert(key, page);
        self.cache_order.push_back(key);
    }

    pub fn get_from_cache(&mut self, key: &CacheKey) -> Option<&CachedPage> {
        if self.page_cache.contains_key(key) {
            // Update LRU order
            self.cache_order.retain(|k| k != key);
            self.cache_order.push_back(*key);
            self.page_cache.get(key)
        } else {
            None
        }
    }

    pub fn remove_document(&mut self, doc_id: DocumentId) {
        self.documents.remove(&doc_id);
        // Remove all cached pages for this document
        self.cache_order.retain(|(id, _, _)| *id != doc_id);
        self.page_cache.retain(|(id, _, _), _| *id != doc_id);
    }
}
