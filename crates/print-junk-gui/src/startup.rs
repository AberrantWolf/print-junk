use eframe::egui;
use serde::{Deserialize, Serialize};

/// Which feature tab is currently active.
#[derive(Default, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Viewer,
    Flashcards,
    Impose,
    Typesetting,
}

impl Mode {
    /// All modes paired with the icon and label used in the tab bar and the
    /// startup selector. Single source of truth so the two stay in sync.
    pub const ALL: [(Mode, &'static str, &'static str); 4] = [
        (Mode::Viewer, "📄", "Viewer"),
        (Mode::Flashcards, "🃏", "Flashcards"),
        (Mode::Impose, "📑", "Impose"),
        (Mode::Typesetting, "📝", "Typesetting"),
    ];
}

/// How the app decides which tab to open on launch.
#[derive(Default, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum StartupBehavior {
    /// Show the startup selector every launch.
    #[default]
    AlwaysAsk,
    /// Open straight into [`StartupSettings::default_mode`].
    AlwaysUse,
    /// Open straight into the last tab that was active.
    UseLast,
}

/// Persisted startup preferences (stored via eframe; works on desktop and web).
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupSettings {
    pub behavior: StartupBehavior,
    /// Remembered choice for [`StartupBehavior::AlwaysUse`].
    pub default_mode: Mode,
    /// Last active tab, updated on save; used by [`StartupBehavior::UseLast`].
    pub last_mode: Mode,
    /// Recently opened/saved project files, most-recent first.
    pub recent_projects: Vec<std::path::PathBuf>,
}

impl StartupSettings {
    /// Record a project path as most-recently used (deduped, capped).
    pub fn push_recent_project(&mut self, path: std::path::PathBuf) {
        self.recent_projects.retain(|p| p != &path);
        self.recent_projects.insert(0, path);
        self.recent_projects.truncate(8);
    }
}

impl StartupSettings {
    /// The tab to open into on launch.
    pub fn initial_mode(&self) -> Mode {
        match self.behavior {
            StartupBehavior::AlwaysAsk | StartupBehavior::UseLast => self.last_mode,
            StartupBehavior::AlwaysUse => self.default_mode,
        }
    }

    /// Whether the startup selector should be shown automatically on launch.
    pub fn should_show_on_launch(&self) -> bool {
        self.behavior == StartupBehavior::AlwaysAsk
    }
}

fn square_button(ui: &mut egui::Ui, emoji: &str, label: &str) -> bool {
    let text = egui::RichText::new(format!("{emoji}\n\n{label}")).size(20.0);
    ui.add_sized([120.0, 120.0], egui::Button::new(text)).clicked()
}

/// Render the startup selector modal. Clicking a big button enters that tab and
/// closes the modal; the radio sets future startup behavior. If "Always use
/// this" is selected, the clicked tab becomes the remembered default.
pub fn show_startup_modal(
    ctx: &egui::Context,
    mode: &mut Mode,
    settings: &mut StartupSettings,
    show: &mut bool,
) {
    let response = egui::Modal::new(egui::Id::new("startup_selector")).show(ctx, |ui| {
        ui.set_width(420.0);

        ui.vertical_centered(|ui| {
            ui.heading("What do you want to do?");
        });
        ui.add_space(12.0);

        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                for (m, emoji, label) in Mode::ALL {
                    if square_button(ui, emoji, label) {
                        *mode = m;
                        if settings.behavior == StartupBehavior::AlwaysUse {
                            settings.default_mode = m;
                        }
                        *show = false;
                    }
                }
            });
        });

        ui.add_space(12.0);
        ui.separator();

        ui.label("On startup:");
        ui.radio_value(
            &mut settings.behavior,
            StartupBehavior::AlwaysAsk,
            "Always ask",
        );
        ui.radio_value(
            &mut settings.behavior,
            StartupBehavior::AlwaysUse,
            "Always use this",
        );
        ui.radio_value(
            &mut settings.behavior,
            StartupBehavior::UseLast,
            "Use last open",
        );
    });

    // Allow dismissing via backdrop click or Escape.
    if response.should_close() {
        *show = false;
    }
}
