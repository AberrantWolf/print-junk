//! The PDF preview, built on the shared `junk-libs-egui-docview` widget in
//! display-only mode.
//!
//! Rendering itself stays on the async worker (`handlers::viewer`, behind the
//! `pdf-viewer` feature): [`PreviewDoc`] is a UI-side [`PageModel`] adapter whose
//! `rerender_page` just sends a [`PdfCommand`], and the decoded result is handed
//! back via [`ViewerState::set_rendered_page`]. So the UI thread never renders,
//! the worker keeps its LRU page cache, and `DocView` provides zoom/pan/nav.

use eframe::egui;
use pdf_async_runtime::{DocumentId, PdfCommand};
use tokio::sync::mpsc;

use junk_libs_egui_docview::{DocView, PageModel, Rect, RegionId};

/// Page size (US Letter, points) assumed before a document's first render lands,
/// so the widget can lay out and request that render. Replaced by the real size
/// as soon as any page renders.
const DEFAULT_PAGE_SIZE: (f32, f32) = (612.0, 792.0);

/// UI-side [`PageModel`] for the preview: a thin adapter over the async worker.
/// It never renders — `rerender_page` sends a command and the result arrives via
/// [`ViewerState::set_rendered_page`]. Display-only, so the region methods are
/// no-ops.
pub struct PreviewDoc {
    doc_id: DocumentId,
    page_count: usize,
    command_tx: mpsc::UnboundedSender<PdfCommand>,
    /// The one page held for display: `(index, RGBA bitmap)`. `DocView` only ever
    /// asks for the current page, so a single slot suffices — the worker keeps the
    /// LRU cache of the rest. `None` until the page's render arrives.
    slot: Option<(usize, image::RgbaImage)>,
    /// Most recent page size in points, used to lay out pages whose bitmap hasn't
    /// arrived yet (PDF pages are usually uniform); a default until first render.
    page_size: (f32, f32),
}

impl PreviewDoc {
    fn new(
        doc_id: DocumentId,
        page_count: usize,
        command_tx: mpsc::UnboundedSender<PdfCommand>,
    ) -> Self {
        Self {
            doc_id,
            page_count,
            command_tx,
            slot: None,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

impl PageModel for PreviewDoc {
    fn page_count(&self) -> usize {
        self.page_count
    }

    fn page_size(&self, _page: usize) -> Option<(f32, f32)> {
        // Always known (the real size once anything has rendered, else a default),
        // so DocView lays out and drives the render request even before the first
        // bitmap arrives. `None` only when the document is empty.
        (self.page_count > 0).then_some(self.page_size)
    }

    fn page_bitmap(&self, page: usize) -> Option<&image::RgbaImage> {
        // Only return the bitmap when it is *this* page's, so a stale slot (after
        // navigating before the new page renders) never textures the wrong page.
        match &self.slot {
            Some((p, img)) if *p == page => Some(img),
            _ => None,
        }
    }

    fn rerender_page(&mut self, page: usize, scale: f32) {
        // Fire-and-forget: ask the worker to render this page. The worker maps
        // `zoom_level` straight to the render scale (pixels per point).
        let _ = self.command_tx.send(PdfCommand::ViewerRenderPage {
            doc_id: self.doc_id,
            page_index: page,
            zoom_level: scale,
        });
    }

    // Display-only: no regions.
    fn regions_on(&self, _page: usize) -> Vec<(RegionId, Rect)> {
        Vec::new()
    }
    fn add_region(&mut self, _page: usize, _rect: Rect) -> RegionId {
        RegionId(0)
    }
    fn region_rect_mut(&mut self, _id: RegionId) -> Option<&mut Rect> {
        None
    }
    fn remove_region(&mut self, _id: RegionId) {}
}

/// State for one preview/viewer instance: the document adapter, the widget's view
/// state, the current page, and whether to offer a Close button.
pub struct ViewerState {
    doc: PreviewDoc,
    view: DocView,
    current_page: usize,
    show_close_button: bool,
}

impl ViewerState {
    pub fn new(
        doc_id: DocumentId,
        page_count: usize,
        show_close_button: bool,
        command_tx: mpsc::UnboundedSender<PdfCommand>,
    ) -> Self {
        Self {
            doc: PreviewDoc::new(doc_id, page_count, command_tx),
            view: DocView::default(),
            current_page: 0,
            show_close_button,
        }
    }

    /// The document this viewer is showing (for routing worker updates).
    pub fn current_doc_id(&self) -> DocumentId {
        self.doc.doc_id
    }

    /// Total pages in the document (for prefetch bounds).
    pub fn total_pages(&self) -> usize {
        self.doc.page_count
    }

    /// Store a freshly rendered page from the worker. `width`/`height` are the
    /// raster size; `size_pts` is the page's native point size (or `(0, 0)` for a
    /// cache hit, in which case the previously known size is kept). Only the
    /// current page is held — the worker caches the rest.
    pub fn set_rendered_page(
        &mut self,
        page_index: usize,
        width: usize,
        height: usize,
        rgba: Vec<u8>,
        size_pts: (f32, f32),
    ) {
        if size_pts.0 > 0.0 && size_pts.1 > 0.0 {
            self.doc.page_size = size_pts;
        }
        if let Some(img) = image::RgbaImage::from_raw(width as u32, height as u32, rgba) {
            self.doc.slot = Some((page_index, img));
        }
    }

    /// Point this viewer at a new document (e.g. a re-imposed preview), keeping
    /// the user's page position where it is still valid. Returns the previous doc
    /// id so the caller can free it in the worker.
    pub fn update_for_new_document(
        &mut self,
        new_doc_id: DocumentId,
        new_page_count: usize,
    ) -> DocumentId {
        let old = self.doc.doc_id;
        self.doc.doc_id = new_doc_id;
        self.doc.page_count = new_page_count;
        self.doc.slot = None;
        self.doc.page_size = DEFAULT_PAGE_SIZE;
        self.view.reset();
        self.current_page = self.current_page.min(new_page_count.saturating_sub(1));
        old
    }
}

/// Render the preview pane. With a document loaded it shows the display-only
/// `DocView`; otherwise it shows an open prompt (the standalone Viewer tab —
/// embedded previews show their own placeholder before calling this).
pub fn show_viewer(
    ui: &mut egui::Ui,
    viewer_state: &mut Option<ViewerState>,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let Some(state) = viewer_state else {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.heading("PDF Viewer");
            ui.add_space(20.0);

            #[cfg(feature = "pdf-viewer")]
            {
                ui.label("Drop a PDF file here or click to open");
                ui.add_space(10.0);
                if ui.button("Open PDF...").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .pick_file()
                {
                    log::info!("Loading PDF: {}", path.display());
                    let _ = command_tx.send(PdfCommand::ViewerLoad { path });
                }
            }

            #[cfg(not(feature = "pdf-viewer"))]
            {
                ui.label("PDF viewing not available in this build");
            }
        });
        return;
    };

    // The Close button (standalone viewer only) goes in the widget's control bar.
    let mut close_clicked = false;
    let show_close = state.show_close_button;
    state
        .view
        .show_readonly(ui, &mut state.doc, &mut state.current_page, |ui| {
            if show_close {
                ui.separator();
                if ui.button("Close PDF").clicked() {
                    close_clicked = true;
                }
            }
        });

    if close_clicked {
        let _ = command_tx.send(PdfCommand::ViewerClose {
            doc_id: state.doc.doc_id,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(w: usize, h: usize) -> Vec<u8> {
        vec![0u8; w * h * 4]
    }

    /// `page_size` must be known before any render (so `DocView` lays out and
    /// drives the first request), then track real sizes, and survive cache hits.
    #[test]
    fn page_size_defaults_then_tracks_renders() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut v = ViewerState::new(DocumentId(1), 3, false, tx);
        assert_eq!(v.doc.page_size(0), Some(DEFAULT_PAGE_SIZE));
        v.set_rendered_page(0, 100, 200, rgba(100, 200), (300.0, 600.0));
        assert_eq!(v.doc.page_size(0), Some((300.0, 600.0)));
        // A cache hit reports size (0, 0); the known size must be kept.
        v.set_rendered_page(1, 100, 200, rgba(100, 200), (0.0, 0.0));
        assert_eq!(v.doc.page_size(1), Some((300.0, 600.0)));
    }

    /// The single slot must only texture its own page — never a stale one after
    /// navigating before the new page's render arrives.
    #[test]
    fn page_bitmap_only_for_current_slot_page() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut v = ViewerState::new(DocumentId(1), 3, false, tx);
        assert!(v.doc.page_bitmap(0).is_none());
        v.set_rendered_page(2, 10, 10, rgba(10, 10), (300.0, 600.0));
        assert!(v.doc.page_bitmap(2).is_some());
        assert!(
            v.doc.page_bitmap(0).is_none(),
            "a stale slot must not texture another page"
        );
    }

    /// `rerender_page` is the fire-and-forget seam: it sends one render command
    /// with the page and scale `DocView` asked for.
    #[test]
    fn rerender_sends_a_render_command() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut v = ViewerState::new(DocumentId(7), 3, false, tx);
        v.doc.rerender_page(2, 3.5);
        match rx.try_recv() {
            Ok(PdfCommand::ViewerRenderPage {
                doc_id,
                page_index,
                zoom_level,
            }) => {
                assert_eq!(doc_id, DocumentId(7));
                assert_eq!(page_index, 2);
                assert!((zoom_level - 3.5).abs() < f32::EPSILON);
            }
            _ => panic!("expected a ViewerRenderPage command"),
        }
    }

    /// Switching documents resets the slot and clamps the page into range.
    #[test]
    fn update_for_new_document_resets_and_clamps() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut v = ViewerState::new(DocumentId(1), 10, false, tx);
        v.current_page = 8;
        v.set_rendered_page(8, 10, 10, rgba(10, 10), (300.0, 600.0));
        let old = v.update_for_new_document(DocumentId(2), 3);
        assert_eq!(old, DocumentId(1));
        assert_eq!(v.current_doc_id(), DocumentId(2));
        assert_eq!(v.total_pages(), 3);
        assert_eq!(v.current_page, 2, "page clamped to the new last page");
        assert!(v.doc.slot.is_none(), "slot cleared for the new document");
    }
}
