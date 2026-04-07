use eframe::egui;
use pdf_async_runtime::{DocumentId, PdfCommand};
use tokio::sync::mpsc;

use crate::viewer::quantize_zoom;

/// Zoom state for the PDF viewer. When present, zoom controls are shown.
#[derive(Clone)]
pub struct ZoomState {
    /// Current zoom as percentage (25.0..=400.0)
    pub zoom_percent: f32,
    /// Whether "fit to window" mode is active
    pub fit_to_window: bool,
    /// Quantized zoom percentage of the currently displayed texture
    pub rendered_zoom: Option<u32>,
    /// Native page dimensions in PDF points (set after first render)
    pub page_native_size: Option<(f32, f32)>,
    /// Last known scroll offset (updated each frame from ScrollArea output)
    last_scroll_offset: egui::Vec2,
    /// Last known viewport size (updated each frame from ScrollArea output)
    last_viewport_size: egui::Vec2,
    /// Override scroll offset for the next frame (set on zoom change, consumed by ScrollArea)
    scroll_offset_override: Option<egui::Vec2>,
}

impl Default for ZoomState {
    fn default() -> Self {
        Self {
            zoom_percent: 100.0,
            fit_to_window: true,
            rendered_zoom: None,
            page_native_size: None,
            last_scroll_offset: egui::Vec2::ZERO,
            last_viewport_size: egui::Vec2::ZERO,
            scroll_offset_override: None,
        }
    }
}

impl ZoomState {
    /// Compute a scroll offset that preserves the viewport center when zoom changes.
    /// `anchor_offset` is the point in viewport coordinates to keep stable
    /// (e.g. viewport_size/2 for center, or cursor position for scroll-zoom).
    fn compute_scroll_for_zoom(&self, old_zoom: f32, new_zoom: f32, anchor_offset: egui::Vec2) -> egui::Vec2 {
        let ratio = new_zoom / old_zoom;
        // The anchor point in content coordinates (before zoom change)
        let anchor_in_content = self.last_scroll_offset + anchor_offset;
        // After zoom, that same logical point moves to anchor_in_content * ratio
        // We want it to still be at anchor_offset within the viewport
        let new_offset = anchor_in_content * ratio - anchor_offset;
        // Clamp to non-negative (can't scroll before content start)
        egui::vec2(new_offset.x.max(0.0), new_offset.y.max(0.0))
    }
}

#[derive(Clone)]
pub struct ViewerState {
    pub current_doc_id: Option<DocumentId>,
    pub current_page: usize,
    pub total_pages: usize,
    pub page_texture: Option<egui::TextureHandle>,
    /// When Some, zoom controls are shown and zoom rendering is active.
    pub zoom: Option<ZoomState>,
    /// Whether to show the "Close PDF" button (false for embedded previews)
    pub show_close_button: bool,
}

impl ViewerState {
    #[allow(dead_code)]
    pub fn new(doc_id: DocumentId, page_count: usize) -> Self {
        Self {
            current_doc_id: Some(doc_id),
            current_page: 0,
            total_pages: page_count,
            page_texture: None,
            zoom: None,
            show_close_button: true,
        }
    }

    /// Get the zoom level as a fraction for render commands.
    /// Returns 0.0 when zoom is disabled (legacy mode).
    pub fn zoom_fraction(&self) -> f32 {
        self.zoom.as_ref().map_or(0.0, |z| z.zoom_percent / 100.0)
    }
}

/// Zoom step: +25% below 100%, +50% above 100%
fn zoom_step_up(current: f32) -> f32 {
    if current < 100.0 {
        (current + 25.0).min(400.0)
    } else {
        (current + 50.0).min(400.0)
    }
}

fn zoom_step_down(current: f32) -> f32 {
    if current <= 100.0 {
        (current - 25.0).max(25.0)
    } else {
        (current - 50.0).max(25.0)
    }
}

/// Navigate the viewer to a specific page and send a render command.
fn navigate_to_page(
    state: &mut ViewerState,
    page: usize,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let page = page.min(state.total_pages.saturating_sub(1));
    if page == state.current_page {
        return;
    }
    state.current_page = page;
    if let Some(doc_id) = state.current_doc_id {
        let _ = command_tx.send(PdfCommand::ViewerRenderPage {
            doc_id,
            page_index: state.current_page,
            zoom_level: state.zoom_fraction(),
        });
        log::info!("Rendering page {}...", state.current_page + 1);
    }
}

pub fn show_viewer(
    ui: &mut egui::Ui,
    viewer_state: &mut Option<ViewerState>,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    if let Some(state) = viewer_state {
        // Show navigation bar
        ui.horizontal(|ui| {
            let can_go_back = state.current_page > 0;
            let can_go_forward = state.current_page < state.total_pages.saturating_sub(1);

            if ui
                .add_enabled(can_go_back, egui::Button::new("◀ Previous"))
                .clicked()
            {
                navigate_to_page(state, state.current_page.saturating_sub(1), command_tx);
            }

            ui.label(format!(
                "Page {} of {}",
                state.current_page + 1,
                state.total_pages
            ));

            if ui
                .add_enabled(can_go_forward, egui::Button::new("Next ▶"))
                .clicked()
            {
                navigate_to_page(state, state.current_page + 1, command_tx);
            }

            // Close button pushed to the right, away from navigation
            if state.show_close_button {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close PDF").clicked() {
                        if let Some(doc_id) = state.current_doc_id {
                            let _ = command_tx.send(PdfCommand::ViewerClose { doc_id });
                        }
                    }
                });
            }
        });

        // Show zoom toolbar if zoom is enabled
        let mut zoom_changed = false;
        let old_zoom_percent = state.zoom.as_ref().map(|z| z.zoom_percent).unwrap_or(100.0);
        if let Some(zoom) = &mut state.zoom {
            ui.horizontal(|ui| {
                // Fit to window button
                if ui
                    .selectable_label(zoom.fit_to_window, "Fit")
                    .clicked()
                {
                    zoom.fit_to_window = true;
                    // Fit zoom will be computed below when we know available size
                    zoom_changed = true;
                }

                ui.separator();

                // Preset buttons
                for preset in [50.0, 100.0, 150.0, 200.0] {
                    let label = format!("{}%", preset as u32);
                    if ui
                        .selectable_label(
                            !zoom.fit_to_window
                                && (zoom.zoom_percent - preset).abs() < 0.5,
                            label,
                        )
                        .clicked()
                    {
                        zoom.zoom_percent = preset;
                        zoom.fit_to_window = false;
                        zoom_changed = true;
                    }
                }

                ui.separator();

                // Zoom out button
                if ui.button("−").clicked() {
                    zoom.zoom_percent = zoom_step_down(zoom.zoom_percent);
                    zoom.fit_to_window = false;
                    zoom_changed = true;
                }

                // Draggable zoom percentage
                let mut pct = zoom.zoom_percent.round() as i32;
                if ui
                    .add(
                        egui::DragValue::new(&mut pct)
                            .range(25..=400)
                            .suffix("%")
                            .speed(1.0),
                    )
                    .changed()
                {
                    zoom.zoom_percent = pct as f32;
                    zoom.fit_to_window = false;
                    zoom_changed = true;
                }

                // Zoom in button
                if ui.button("+").clicked() {
                    zoom.zoom_percent = zoom_step_up(zoom.zoom_percent);
                    zoom.fit_to_window = false;
                    zoom_changed = true;
                }
            });
        }

        ui.separator();

        // Compute fit-to-window zoom before rendering the scroll area
        if let Some(zoom) = &mut state.zoom {
            if zoom.fit_to_window {
                let available = ui.available_size();
                if let Some((pw, ph)) = zoom.page_native_size {
                    if pw > 0.0 && ph > 0.0 {
                        let zoom_w = available.x / pw;
                        let zoom_h = available.y / ph;
                        let fit_percent = zoom_w.min(zoom_h) * 100.0;
                        let fit_percent = fit_percent.clamp(25.0, 400.0);
                        if (fit_percent - zoom.zoom_percent).abs() > 1.0 {
                            zoom.zoom_percent = fit_percent;
                            zoom_changed = true;
                        }
                    }
                }
            }
        }

        // Handle keyboard zoom shortcuts (Cmd/Ctrl + =/-/0)
        if state.zoom.is_some() {
            let cmd_held = ui.input(|i| i.modifiers.command);
            if cmd_held {
                let fit_pressed = ui.input(|i| i.key_pressed(egui::Key::Num0));
                let plus_pressed = ui.input(|i| {
                    i.key_pressed(egui::Key::Equals) || i.key_pressed(egui::Key::Plus)
                });
                let minus_pressed = ui.input(|i| i.key_pressed(egui::Key::Minus));

                if let Some(zoom) = &mut state.zoom {
                    if fit_pressed {
                        zoom.fit_to_window = true;
                        zoom_changed = true;
                    } else if plus_pressed {
                        zoom.zoom_percent = zoom_step_up(zoom.zoom_percent);
                        zoom.fit_to_window = false;
                        zoom_changed = true;
                    } else if minus_pressed {
                        zoom.zoom_percent = zoom_step_down(zoom.zoom_percent);
                        zoom.fit_to_window = false;
                        zoom_changed = true;
                    }
                }
            }
        }

        // Handle Ctrl+scroll zoom (anchored on cursor position)
        if state.zoom.is_some() {
            let scroll_zoom = ui.input(|i| {
                if i.modifiers.command {
                    let delta = i.smooth_scroll_delta.y;
                    if delta.abs() > 0.1 {
                        // Get cursor position relative to the scroll area
                        let pointer_pos = i.pointer.hover_pos();
                        Some((delta, pointer_pos))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
            if let Some((delta, pointer_pos)) = scroll_zoom {
                if let Some(zoom) = &mut state.zoom {
                    let old_pct = zoom.zoom_percent;
                    // Scale zoom by scroll delta (positive = zoom in)
                    let change = delta * 0.5; // sensitivity
                    zoom.zoom_percent = (zoom.zoom_percent + change).clamp(25.0, 400.0);
                    zoom.fit_to_window = false;
                    zoom_changed = true;

                    // Anchor zoom on cursor position if available
                    if let Some(pos) = pointer_pos {
                        // Convert screen position to viewport-relative position
                        // by subtracting the scroll area's top-left (approximated by
                        // current ui clip rect min, which is close enough)
                        let viewport_anchor = pos - ui.clip_rect().min;
                        let viewport_anchor = egui::vec2(
                            viewport_anchor.x.max(0.0),
                            viewport_anchor.y.max(0.0),
                        );
                        zoom.scroll_offset_override = Some(
                            zoom.compute_scroll_for_zoom(old_pct, zoom.zoom_percent, viewport_anchor),
                        );
                    }
                }
            }
        }

        // Handle page navigation keyboard shortcuts
        {
            let cmd_held = ui.input(|i| i.modifiers.command);
            if !cmd_held {
                let prev = ui.input(|i| {
                    i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::PageUp)
                });
                let next = ui.input(|i| {
                    i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::PageDown)
                });
                let first = ui.input(|i| i.key_pressed(egui::Key::Home));
                let last = ui.input(|i| i.key_pressed(egui::Key::End));

                if first {
                    navigate_to_page(state, 0, command_tx);
                } else if last {
                    navigate_to_page(state, state.total_pages.saturating_sub(1), command_tx);
                } else if prev {
                    navigate_to_page(state, state.current_page.saturating_sub(1), command_tx);
                } else if next {
                    navigate_to_page(state, state.current_page + 1, command_tx);
                }
            }
        }

        // Compute scroll position preservation when zoom changes.
        // Skip if Ctrl+scroll already set a cursor-anchored override.
        if zoom_changed {
            if let Some(zoom) = &mut state.zoom {
                if zoom.scroll_offset_override.is_none() {
                    let new_zoom = zoom.zoom_percent;
                    if (new_zoom - old_zoom_percent).abs() > 0.1 {
                        // For button/keyboard zoom, anchor on viewport center
                        let anchor = zoom.last_viewport_size * 0.5;
                        zoom.scroll_offset_override = Some(
                            zoom.compute_scroll_for_zoom(old_zoom_percent, new_zoom, anchor),
                        );
                    }
                }
            }
        }

        // Send render command if zoom changed and quantized level differs
        if zoom_changed {
            if let (Some(doc_id), Some(zoom)) = (state.current_doc_id, &mut state.zoom) {
                let new_quantized = quantize_zoom(zoom.zoom_percent / 100.0);
                if zoom.rendered_zoom != Some(new_quantized) {
                    let _ = command_tx.send(PdfCommand::ViewerRenderPage {
                        doc_id,
                        page_index: state.current_page,
                        zoom_level: zoom.zoom_percent / 100.0,
                    });
                }
            }
        }

        // Display page texture if available
        if let Some(texture) = &state.page_texture {
            // Compute display size based on zoom
            let display_size = if let Some(zoom) = &state.zoom {
                let scale = zoom.zoom_percent / 100.0;
                if let Some((pw, ph)) = zoom.page_native_size {
                    egui::vec2(pw * scale, ph * scale)
                } else {
                    texture.size_vec2()
                }
            } else {
                texture.size_vec2()
            };

            // Apply scroll offset override if a zoom change requested it
            let mut scroll_area = egui::ScrollArea::both();
            if let Some(zoom) = &mut state.zoom {
                if let Some(offset) = zoom.scroll_offset_override.take() {
                    scroll_area = scroll_area
                        .horizontal_scroll_offset(offset.x)
                        .vertical_scroll_offset(offset.y);
                }
            }

            let scroll_output = scroll_area.show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.image(egui::load::SizedTexture::new(texture.id(), display_size));
                });
            });

            // Record scroll state for position preservation on next zoom change
            if let Some(zoom) = &mut state.zoom {
                zoom.last_scroll_offset = scroll_output.state.offset;
                zoom.last_viewport_size = scroll_output.inner_rect.size();
            }
        } else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.label("Rendering page...");
            });
        }
    } else {
        // No PDF loaded - show file loading UI
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.heading("PDF Viewer");
            ui.add_space(20.0);

            #[cfg(feature = "pdf-viewer")]
            {
                ui.label("Drop a PDF file here or click to open");
                ui.add_space(10.0);

                if ui.button("Open PDF...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .pick_file()
                    {
                        log::info!("Loading PDF: {}", path.display());
                        let _ = command_tx.send(PdfCommand::ViewerLoad { path });
                    }
                }
            }

            #[cfg(not(feature = "pdf-viewer"))]
            {
                ui.label("PDF viewing not available in WASM build");
            }
        });
    }
}
