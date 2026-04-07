use eframe::egui;
use pdf_async_runtime::{PdfCommand, PdfUpdate};
use tokio::sync::mpsc;

use crate::logger::AppLogger;
use crate::views::{
    FlashcardState, ImposeState, ViewerState, ZoomState, show_flashcards, show_impose, show_viewer,
};

#[derive(Default, PartialEq)]
enum Mode {
    #[default]
    Viewer,
    Flashcards,
    Impose,
}

#[derive(Clone)]
struct ProgressState {
    operation: String,
    current: usize,
    total: usize,
}

pub struct PdfToolsApp {
    mode: Mode,

    // Logging
    logger: AppLogger,
    log_viewer_open: bool,

    // Async infrastructure
    command_tx: mpsc::UnboundedSender<PdfCommand>,
    update_rx: mpsc::UnboundedReceiver<PdfUpdate>,

    // Progress tracking
    progress: Option<ProgressState>,

    // Feature state
    flashcard_state: FlashcardState,
    viewer_state: Option<ViewerState>,
    impose_state: ImposeState,

    // Runtime handle (native only)
    #[cfg(not(target_arch = "wasm32"))]
    _tokio_handle: tokio::runtime::Handle,
}

impl PdfToolsApp {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(_cc: &eframe::CreationContext<'_>, tokio_handle: tokio::runtime::Handle) -> Self {
        let logger = AppLogger::new(1000);
        logger.clone().init().expect("Failed to initialize logger");

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (worker_update_tx, mut worker_update_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Forward worker updates to the app channel, requesting repaint on each
        let repaint_ctx = _cc.egui_ctx.clone();
        tokio_handle.spawn(async move {
            while let Some(update) = worker_update_rx.recv().await {
                if update_tx.send(update).is_err() {
                    break;
                }
                repaint_ctx.request_repaint();
            }
        });

        // Spawn worker task
        tokio_handle.spawn(crate::worker::worker_task(command_rx, worker_update_tx));

        log::info!("PDF Tools GUI started");

        Self {
            mode: Mode::default(),
            logger,
            log_viewer_open: false,
            command_tx,
            update_rx,
            progress: None,
            flashcard_state: FlashcardState::default(),
            viewer_state: None,
            impose_state: ImposeState::default(),
            _tokio_handle: tokio_handle,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let logger = AppLogger::new(1000);
        logger.clone().init().expect("Failed to initialize logger");

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (worker_update_tx, mut worker_update_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Forward worker updates to the app channel, requesting repaint on each
        let repaint_ctx = _cc.egui_ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            while let Some(update) = worker_update_rx.recv().await {
                if update_tx.send(update).is_err() {
                    break;
                }
                repaint_ctx.request_repaint();
            }
        });

        // Spawn worker task using wasm-bindgen-futures
        wasm_bindgen_futures::spawn_local(crate::worker::worker_task(command_rx, worker_update_tx));

        log::info!("PDF Tools GUI started");

        Self {
            mode: Mode::default(),
            logger,
            log_viewer_open: false,
            command_tx,
            update_rx,
            progress: None,
            flashcard_state: FlashcardState::default(),
            viewer_state: None,
            impose_state: ImposeState::default(),
        }
    }
}

impl eframe::App for PdfToolsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle drag-and-drop routed by current mode
        let dropped: Vec<_> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for path in dropped {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            match self.mode {
                Mode::Impose if ext == "pdf" => {
                    if !self.impose_state.options.input_files.contains(&path) {
                        log::info!("Adding PDF to impose inputs: {}", path.display());
                        self.impose_state.options.input_files.push(path);
                        self.impose_state.needs_regeneration = true;
                    }
                }
                Mode::Flashcards if ext == "csv" => {
                    log::info!("Loading CSV: {}", path.display());
                    self.flashcard_state.csv_path = path.display().to_string();
                    let _ = self
                        .command_tx
                        .send(PdfCommand::FlashcardsLoadCsv { input_path: path });
                }
                _ if ext == "pdf" => {
                    log::info!("Loading PDF: {}", path.display());
                    let _ = self
                        .command_tx
                        .send(PdfCommand::ViewerLoad { path });
                }
                _ => {}
            }
        }

        // Global keyboard shortcuts: Cmd+O (open), Cmd+S (save)
        #[cfg(not(target_arch = "wasm32"))]
        {
            let cmd_o = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O));
            let cmd_s = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S));

            if cmd_o {
                match self.mode {
                    Mode::Viewer => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .pick_file()
                        {
                            log::info!("Loading PDF: {}", path.display());
                            let _ = self.command_tx.send(PdfCommand::ViewerLoad { path });
                        }
                    }
                    Mode::Impose => {
                        if let Some(paths) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .pick_files()
                        {
                            for path in paths {
                                if !self.impose_state.options.input_files.contains(&path) {
                                    self.impose_state.options.input_files.push(path);
                                    self.impose_state.needs_regeneration = true;
                                }
                            }
                        }
                    }
                    Mode::Flashcards => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .pick_file()
                        {
                            self.flashcard_state.csv_path = path.display().to_string();
                            log::info!("Loading CSV: {}", path.display());
                            let _ = self
                                .command_tx
                                .send(PdfCommand::FlashcardsLoadCsv { input_path: path });
                        }
                    }
                }
            }

            if cmd_s {
                match self.mode {
                    Mode::Impose if !self.impose_state.options.input_files.is_empty() => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .set_file_name("imposed.pdf")
                            .save_file()
                        {
                            log::info!("Saving imposed PDF to: {}", path.display());
                            let _ = self.command_tx.send(PdfCommand::ImposeGenerate {
                                options: self.impose_state.options.clone(),
                                output_path: path,
                            });
                        }
                    }
                    Mode::Flashcards if !self.flashcard_state.cards.is_empty() => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .set_file_name("flashcards.pdf")
                            .save_file()
                        {
                            log::info!("Saving flashcards to: {}", path.display());
                            let options = self.flashcard_state.to_options();
                            let _ = self.command_tx.send(PdfCommand::FlashcardsGenerate {
                                cards: self.flashcard_state.cards.clone(),
                                options,
                                output_path: path,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        // Process all pending updates from worker
        while let Ok(update) = self.update_rx.try_recv() {
            match update {
                PdfUpdate::Progress {
                    operation,
                    current,
                    total,
                } => {
                    self.progress = Some(ProgressState {
                        operation,
                        current,
                        total,
                    });
                }
                PdfUpdate::FlashcardsLoaded { cards } => {
                    log::info!("Loaded {} flashcards from CSV", cards.len());
                    self.progress = None;
                    self.flashcard_state.cards = cards;
                    self.flashcard_state.needs_regeneration = true;
                }
                PdfUpdate::FlashcardsComplete { path, card_count } => {
                    log::info!("Generated {} flashcards → {}", card_count, path.display());
                    self.progress = None;

                    // Load preview if it's a temp file
                    if path.starts_with(std::env::temp_dir()) {
                        let _ = self.command_tx.send(PdfCommand::ViewerLoad { path });
                    }
                }
                PdfUpdate::ImposeLoaded { doc_id, page_count } => {
                    log::info!("Loaded PDF with {} pages (ID: {:?})", page_count, doc_id);
                    self.progress = None;
                }
                PdfUpdate::ImposeComplete { path } => {
                    log::info!("Imposed PDF → {}", path.display());
                    self.progress = None;

                    // Load preview if it's a temp file
                    if path.starts_with(std::env::temp_dir()) {
                        let _ = self.command_tx.send(PdfCommand::ViewerLoad { path });
                    }
                }
                PdfUpdate::ImposePreviewGenerated {
                    pdf_bytes,
                    page_count,
                    signatures_shown,
                    total_signatures,
                } => {
                    log::info!("Preview generated with {} pages ({} of {} signatures)",
                        page_count, signatures_shown, total_signatures);
                    self.impose_state.preview_page_count = page_count;
                    self.impose_state.preview_signatures_shown = Some(signatures_shown);
                    self.impose_state.preview_total_signatures = Some(total_signatures);
                    self.progress = None;

                    // Load the preview bytes into the viewer (no disk round-trip)
                    let _ = self
                        .command_tx
                        .send(PdfCommand::ViewerLoadBytes { pdf_bytes, page_count });
                }
                PdfUpdate::ImposeConfigLoaded { options } => {
                    log::info!("Configuration loaded");
                    self.impose_state.options = options.clone();
                    self.progress = None;

                    // Recalculate stats with new options
                    let _ = self
                        .command_tx
                        .send(PdfCommand::ImposeCalculateStats { options });
                }
                PdfUpdate::ImposeStatsCalculated { stats } => {
                    self.impose_state.stats = Some(stats);
                }
                PdfUpdate::Error { message } => {
                    log::error!("Error: {}", message);
                    self.progress = None;
                }
                PdfUpdate::ViewerLoaded { doc_id, page_count } => {
                    let is_standalone_viewer = matches!(self.mode, Mode::Viewer);
                    let new_viewer_state = ViewerState {
                        current_doc_id: Some(doc_id),
                        current_page: 0,
                        total_pages: page_count,
                        page_texture: None,
                        zoom: Some(ZoomState::default()),
                        show_close_button: is_standalone_viewer,
                    };

                    // Update viewer state based on current mode
                    match self.mode {
                        Mode::Flashcards => {
                            self.flashcard_state.preview_viewer = Some(new_viewer_state.clone());
                        }
                        Mode::Viewer => {
                            self.viewer_state = Some(new_viewer_state.clone());
                        }
                        Mode::Impose => {
                            self.impose_state.preview_viewer = Some(new_viewer_state.clone());
                        }
                    };
                    // First render at 100%; fit-to-window will adjust on next frame
                    let zoom_level = 1.0;

                    log::info!("Loaded PDF with {} pages", page_count);
                    self.progress = None;

                    // Request render of first page
                    let _ = self.command_tx.send(PdfCommand::ViewerRenderPage {
                        doc_id,
                        page_index: 0,
                        zoom_level,
                    });
                }
                PdfUpdate::ViewerPageRendered {
                    doc_id,
                    page_index,
                    rgba_data,
                    width,
                    height,
                    zoom_level,
                    page_width_pts,
                    page_height_pts,
                } => {
                    let color_image =
                        egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba_data);

                    // Update the appropriate viewer state
                    if let Some(state) = &mut self.viewer_state {
                        if let Some(texture) = &mut state.page_texture {
                            texture.set(color_image.clone(), egui::TextureOptions::default());
                        } else {
                            state.page_texture = Some(ctx.load_texture(
                                "pdf_page",
                                color_image.clone(),
                                egui::TextureOptions::default(),
                            ));
                        }
                        // Update zoom state with native page dimensions and rendered zoom
                        if let Some(zoom) = &mut state.zoom {
                            if page_width_pts > 0.0 && page_height_pts > 0.0 {
                                zoom.page_native_size = Some((page_width_pts, page_height_pts));
                            }
                            zoom.rendered_zoom = Some(crate::viewer::quantize_zoom(zoom_level));
                        }
                    }

                    for (name, preview) in [
                        ("flashcard_preview", &mut self.flashcard_state.preview_viewer),
                        ("impose_preview", &mut self.impose_state.preview_viewer),
                    ] {
                        if let Some(state) = preview {
                            if let Some(texture) = &mut state.page_texture {
                                texture.set(color_image.clone(), egui::TextureOptions::default());
                            } else {
                                state.page_texture = Some(ctx.load_texture(
                                    name,
                                    color_image.clone(),
                                    egui::TextureOptions::default(),
                                ));
                            }
                            if let Some(zoom) = &mut state.zoom {
                                if page_width_pts > 0.0 && page_height_pts > 0.0 {
                                    zoom.page_native_size =
                                        Some((page_width_pts, page_height_pts));
                                }
                                zoom.rendered_zoom =
                                    Some(crate::viewer::quantize_zoom(zoom_level));
                            }
                        }
                    }

                    // Prefetch adjacent pages for faster navigation
                    let total_pages = self
                        .viewer_state
                        .as_ref()
                        .map(|s| s.total_pages)
                        .or_else(|| {
                            self.flashcard_state
                                .preview_viewer
                                .as_ref()
                                .map(|s| s.total_pages)
                        })
                        .or_else(|| {
                            self.impose_state
                                .preview_viewer
                                .as_ref()
                                .map(|s| s.total_pages)
                        })
                        .unwrap_or(0);

                    let mut prefetch_pages = Vec::new();
                    if page_index > 0 {
                        prefetch_pages.push(page_index - 1);
                    }
                    if page_index + 1 < total_pages {
                        prefetch_pages.push(page_index + 1);
                    }
                    // Also prefetch 2 pages ahead for smoother forward navigation
                    if page_index + 2 < total_pages {
                        prefetch_pages.push(page_index + 2);
                    }

                    if !prefetch_pages.is_empty() {
                        let _ = self.command_tx.send(PdfCommand::ViewerPrefetchPages {
                            doc_id,
                            page_indices: prefetch_pages,
                            zoom_level,
                        });
                    }

                    self.progress = None;
                }
                PdfUpdate::ViewerClosed { .. } => {
                    self.viewer_state = None;
                    log::info!("Closed PDF");
                }
            }
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, Mode::Viewer, "📄 Viewer");
                ui.selectable_value(&mut self.mode, Mode::Flashcards, "🃏 Flashcards");
                ui.selectable_value(&mut self.mode, Mode::Impose, "📑 Impose");
            });
        });

        // Status bar at bottom
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Show progress bar
                if let Some(ref progress) = self.progress {
                    ui.label(&progress.operation);
                    ui.add(
                        egui::ProgressBar::new(
                            progress.current as f32 / progress.total.max(1) as f32,
                        )
                        .show_percentage(),
                    );
                    ctx.request_repaint(); // Keep updating during operations
                } else if let Some(latest) = self.logger.latest_message() {
                    if ui.link(&latest).clicked() {
                        self.log_viewer_open = true;
                    }
                }
            });
        });

        // Log viewer window
        egui::Window::new("Log Viewer")
            .open(&mut self.log_viewer_open)
            .default_size([800.0, 400.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Application Logs");
                    if ui.button("Clear").clicked() {
                        self.logger.clear();
                    }
                });

                ui.separator();

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        let entries = self.logger.get_entries();

                        for entry in entries.iter().rev() {
                            ui.horizontal(|ui| {
                                // Timestamp
                                ui.label(
                                    egui::RichText::new(
                                        entry.timestamp.format("%H:%M:%S%.3f").to_string(),
                                    )
                                    .monospace()
                                    .color(egui::Color32::GRAY),
                                );

                                // Level with color
                                let (level_text, level_color) = match entry.level {
                                    log::Level::Error => {
                                        ("ERROR", egui::Color32::from_rgb(255, 80, 80))
                                    }
                                    log::Level::Warn => {
                                        ("WARN ", egui::Color32::from_rgb(255, 200, 80))
                                    }
                                    log::Level::Info => {
                                        ("INFO ", egui::Color32::from_rgb(80, 200, 255))
                                    }
                                    log::Level::Debug => {
                                        ("DEBUG", egui::Color32::from_rgb(200, 200, 200))
                                    }
                                    log::Level::Trace => {
                                        ("TRACE", egui::Color32::from_rgb(150, 150, 150))
                                    }
                                };

                                ui.label(
                                    egui::RichText::new(level_text)
                                        .monospace()
                                        .color(level_color),
                                );

                                // Module
                                ui.label(
                                    egui::RichText::new(&entry.target)
                                        .monospace()
                                        .color(egui::Color32::from_rgb(150, 150, 255)),
                                );

                                // Message
                                ui.label(&entry.message);
                            });
                        }
                    });
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.mode {
            Mode::Viewer => show_viewer(ui, &mut self.viewer_state, &self.command_tx),
            Mode::Flashcards => show_flashcards(ui, &mut self.flashcard_state, &self.command_tx),
            Mode::Impose => show_impose(ui, &mut self.impose_state, &self.command_tx),
        });
    }
}
