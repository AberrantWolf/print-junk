//! Shared page number rendering for imposed pages
//!
//! Page numbers are positioned relative to each page's actual `content_rect`,
//! which is already correctly placed by the layout system. This ensures
//! correct positioning regardless of trim allowance or mixed page sizes.

use crate::constants::{HELVETICA_CHAR_WIDTH_RATIO, PAGE_NUMBER_FONT_SIZE, PAGE_NUMBER_OFFSET};
use crate::layout::PagePlacement;
use lopdf::{Dictionary, Document, Object, ObjectId};

/// Render page numbers onto an imposed page.
///
/// Positions each number centered horizontally within its `content_rect`,
/// at the bottom (non-rotated) or top (rotated) edge with a small offset.
///
/// Returns (content stream operations, font object ID).
pub(crate) fn render_page_numbers(
    output: &mut Document,
    placements: &[PagePlacement],
    page_number_start: usize,
) -> (String, ObjectId) {
    let mut font_dict = Dictionary::new();
    font_dict.set("Type", Object::Name(b"Font".to_vec()));
    font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
    font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
    let font_id = output.add_object(font_dict);

    let mut ops = String::new();

    for placement in placements {
        if let Some(source_idx) = placement.source_page {
            let page_num = page_number_start + source_idx;
            let page_num_text = page_num.to_string();
            let rect = &placement.content_rect;

            if placement.is_rotated() {
                // Rotated 180°: text appears at top of content area (which is visually bottom)
                let text_x = rect.x + rect.width / 2.0;
                let text_y = rect.y + rect.height - PAGE_NUMBER_OFFSET;
                ops.push_str(&format!(
                    "q 1 0 0 1 {} {} cm -1 0 0 -1 0 0 cm BT /F1 {} Tf ({}) Tj ET Q\n",
                    text_x, text_y, PAGE_NUMBER_FONT_SIZE, page_num_text
                ));
            } else {
                let text_width =
                    page_num_text.len() as f32 * PAGE_NUMBER_FONT_SIZE * HELVETICA_CHAR_WIDTH_RATIO;
                let text_x = rect.x + rect.width / 2.0 - text_width / 2.0;
                let text_y = rect.y + PAGE_NUMBER_OFFSET;
                ops.push_str(&format!(
                    "BT /F1 {} Tf {} {} Td ({}) Tj ET\n",
                    PAGE_NUMBER_FONT_SIZE, text_x, text_y, page_num_text
                ));
            }
        }
    }

    (ops, font_id)
}
