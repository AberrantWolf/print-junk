//! Project save/restore.
//!
//! A "project" captures every mode's settings — and the *paths* of any loaded
//! files, never their contents — so work isn't lost between sessions. Two ways
//! to persist:
//!
//! - **Auto-persist**: the whole project is written to eframe storage on exit and
//!   restored on launch (see [`crate::app`]).
//! - **Project files**: an explicit `.pjproj` (JSON) the user can save and reopen,
//!   with a recent-projects list.
//!
//! Desktop-only: the typesetting settings reference the desktop-only `pdf-typeset`
//! types, so the whole feature is excluded from the WASM build.

use std::path::Path;

use pdf_async_runtime::ImpositionOptions;
use serde::{Deserialize, Serialize};

use crate::views::{FlashcardState, TypesettingState};

/// Recommended extension for saved project files.
pub const PROJECT_EXTENSION: &str = "pjproj";

/// Borrowed view of the app's persistable settings, for serialization. Transient
/// fields (loaded cards, preview textures, source text) are `#[serde(skip)]`'d on
/// the underlying state structs, so only settings + file paths are written.
#[derive(Serialize)]
pub struct ProjectRef<'a> {
    pub flashcards: &'a FlashcardState,
    pub impose: &'a ImpositionOptions,
    pub typesetting: &'a TypesettingState,
}

/// Owned, deserialized project settings.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct AppProject {
    pub flashcards: FlashcardState,
    pub impose: ImpositionOptions,
    pub typesetting: TypesettingState,
}

/// Serialize the current settings to pretty JSON.
pub fn to_json(project: &ProjectRef) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(project)
}

/// Parse project settings from JSON.
pub fn from_json(json: &str) -> Result<AppProject, serde_json::Error> {
    serde_json::from_str(json)
}

/// Read and parse a project file.
pub fn read_file(path: &Path) -> std::io::Result<AppProject> {
    let json = std::fs::read_to_string(path)?;
    from_json(&json).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Write the current settings to a project file.
pub fn write_file(path: &Path, project: &ProjectRef) -> std::io::Result<()> {
    let json = to_json(project).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_persists_settings_and_paths_but_not_contents() {
        let flashcards = FlashcardState {
            csv_path: "/tmp/cards.csv".to_string(),
            rows: 4,
            ..Default::default()
        };

        let impose = ImpositionOptions::default();

        let typesetting = TypesettingState {
            source_path: Some("/tmp/book.md".into()),
            source_text: "SECRET CONTENTS".to_string(),
            config: pdf_typeset::TypesetConfig {
                margin_inner_mm: 25.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let json = to_json(&ProjectRef {
            flashcards: &flashcards,
            impose: &impose,
            typesetting: &typesetting,
        })
        .unwrap();

        // File paths are persisted; file *contents* are not.
        assert!(json.contains("/tmp/cards.csv"));
        assert!(json.contains("/tmp/book.md"));
        assert!(!json.contains("SECRET CONTENTS"));

        let loaded = from_json(&json).unwrap();
        assert_eq!(loaded.flashcards.csv_path, "/tmp/cards.csv");
        assert_eq!(loaded.flashcards.rows, 4);
        assert_eq!(loaded.typesetting.source_path, Some("/tmp/book.md".into()));
        assert!(loaded.typesetting.source_text.is_empty());
        assert!((loaded.typesetting.config.margin_inner_mm - 25.0).abs() < f32::EPSILON);
    }
}
