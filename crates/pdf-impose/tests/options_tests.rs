use pdf_impose::*;
use std::path::PathBuf;

fn valid_options() -> ImpositionOptions {
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options
}

#[test]
fn test_validation_no_input_files() {
    let options = ImpositionOptions::default();
    let result = options.validate();
    assert!(result.is_err());
    match result {
        Err(ImposeError::Config(msg)) => {
            assert!(msg.contains("No input files"));
        }
        _ => panic!("Expected Config error"),
    }
}

#[test]
fn test_validation_sheets_per_signature() {
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));

    // Valid: all standard arrangements with 1 sheet
    for arrangement in [
        PageArrangement::Folio,
        PageArrangement::Quarto,
        PageArrangement::Octavo,
    ] {
        options.page_arrangement = arrangement;
        options.sheets_per_signature = 1;
        assert!(options.validate().is_ok());
    }

    // Valid: folio with 3 sheets (12 pages per signature)
    options.page_arrangement = PageArrangement::Folio;
    options.sheets_per_signature = 3;
    assert!(options.validate().is_ok());

    // Invalid: 0 sheets
    options.sheets_per_signature = 0;
    assert!(options.validate().is_err());
}

#[cfg(feature = "serde")]
#[tokio::test]
async fn test_save_and_load_options() {
    use tempfile::NamedTempFile;

    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("input.pdf"));
    options.binding_type = BindingType::PerfectBinding;
    options.page_arrangement = PageArrangement::Octavo;
    options.sheets_per_signature = 2;
    options.output_paper_size = PaperSize::A4;
    options.front_flyleaves = 2;
    options.back_flyleaves = 1;
    options.add_page_numbers = true;

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Save
    options.save(path).await.unwrap();

    // Load
    let loaded = ImpositionOptions::load(path).await.unwrap();

    assert_eq!(loaded.input_files, options.input_files);
    assert_eq!(loaded.binding_type, options.binding_type);
    assert_eq!(loaded.page_arrangement, options.page_arrangement);
    assert_eq!(loaded.sheets_per_signature, options.sheets_per_signature);
    assert_eq!(loaded.output_paper_size, options.output_paper_size);
    assert_eq!(loaded.front_flyleaves, options.front_flyleaves);
    assert_eq!(loaded.back_flyleaves, options.back_flyleaves);
    assert_eq!(loaded.add_page_numbers, options.add_page_numbers);
}

// =============================================================================
// Sizing Validation Tests
// =============================================================================

#[test]
fn test_validation_custom_paper_too_small() {
    let mut options = valid_options();
    options.output_paper_size = PaperSize::Custom {
        width_mm: 5.0,
        height_mm: 5.0,
    };
    let result = options.validate();
    assert!(result.is_err());
    match result {
        Err(ImposeError::Config(msg)) => {
            assert!(msg.contains("at least 10mm"));
        }
        _ => panic!("Expected Config error about paper size"),
    }
}

#[test]
fn test_validation_custom_paper_one_dimension_too_small() {
    let mut options = valid_options();
    options.output_paper_size = PaperSize::Custom {
        width_mm: 100.0,
        height_mm: 5.0,
    };
    assert!(options.validate().is_err());

    options.output_paper_size = PaperSize::Custom {
        width_mm: 5.0,
        height_mm: 100.0,
    };
    assert!(options.validate().is_err());
}

#[test]
fn test_validation_sheet_margins_too_large() {
    let mut options = valid_options();
    options.output_paper_size = PaperSize::Letter; // 215.9mm x 279.4mm landscape
    options.margins.sheet = SheetMargins {
        left_mm: 150.0,
        right_mm: 150.0,
        top_mm: 0.0,
        bottom_mm: 0.0,
    };
    let result = options.validate();
    assert!(result.is_err());
    match result {
        Err(ImposeError::Config(msg)) => {
            assert!(msg.contains("Sheet margins"));
        }
        _ => panic!("Expected Config error about sheet margins"),
    }
}

#[test]
fn test_validation_leaf_margins_too_large_folio() {
    let mut options = valid_options();
    options.page_arrangement = PageArrangement::Folio;
    options.output_paper_size = PaperSize::A5; // 148mm x 210mm
    options.output_orientation = Orientation::Landscape; // 210mm x 148mm
    options.margins.sheet = SheetMargins::none();
    // Folio: each page is half the width = 105mm
    // spine + fore_edge must be < 105mm in points
    options.margins.leaf = LeafMargins {
        spine_mm: 60.0,
        fore_edge_mm: 60.0,
        top_mm: 0.0,
        bottom_mm: 0.0,
        trim_allowance_mm: 0.0,
    };
    let result = options.validate();
    assert!(result.is_err());
    match result {
        Err(ImposeError::Config(msg)) => {
            assert!(msg.contains("Margins are too large"));
        }
        _ => panic!("Expected Config error about margins"),
    }
}

#[test]
fn test_validation_leaf_margins_ok_for_folio_but_too_large_for_octavo() {
    let mut options = valid_options();
    options.output_paper_size = PaperSize::Letter;
    options.output_orientation = Orientation::Landscape;
    options.margins.sheet = SheetMargins::uniform(5.0);
    // Moderate margins that fit folio but not octavo
    // Letter landscape: ~279mm x ~216mm, minus 10mm sheet margins = ~269mm x ~206mm
    // Folio page width: ~134mm, so spine+fore_edge < 134mm — 40+40=80mm fits
    // Octavo page width: ~67mm, so spine+fore_edge < 67mm — 40+40=80mm doesn't fit
    options.margins.leaf = LeafMargins {
        spine_mm: 40.0,
        fore_edge_mm: 40.0,
        top_mm: 0.0,
        bottom_mm: 0.0,
        trim_allowance_mm: 0.0,
    };

    options.page_arrangement = PageArrangement::Folio;
    assert!(
        options.validate().is_ok(),
        "Folio should fit with these margins"
    );

    options.page_arrangement = PageArrangement::Octavo;
    assert!(
        options.validate().is_err(),
        "Octavo should not fit with these margins"
    );
}

#[test]
fn test_validation_reasonable_margins_all_arrangements() {
    for arrangement in [
        PageArrangement::Folio,
        PageArrangement::Quarto,
        PageArrangement::Octavo,
    ] {
        let mut options = valid_options();
        options.page_arrangement = arrangement;
        options.output_paper_size = PaperSize::Letter;
        options.output_orientation = Orientation::Landscape;
        options.margins.sheet = SheetMargins::uniform(5.0);
        options.margins.leaf = LeafMargins {
            spine_mm: 5.0,
            fore_edge_mm: 3.0,
            top_mm: 3.0,
            bottom_mm: 3.0,
            trim_allowance_mm: 2.0,
        };
        assert!(
            options.validate().is_ok(),
            "Reasonable margins should pass for {:?}",
            arrangement
        );
    }
}
