use rikka_image_edit::{Invert, Recipe};
use rikka_types::Color;

use crate::support::*;
use paint_document::{Action, Area, PixelChange};

const YELLOW: [u8; 4] = [255, 255, 0, 255];

fn batch(name: &str, edits: Vec<Edit>) -> Edit {
    Edit::Batch {
        name: name.to_owned(),
        edits,
    }
}

fn invert(layer: LayerId) -> Edit {
    Edit::Develop {
        layer,
        recipe: Recipe::new().then(Invert),
    }
}

fn select_first_pixel() -> Edit {
    Edit::Select {
        region: Region::rectangle(Area::new(0, 0, 1, 1)),
        by: SelectionAction::Rectangle,
    }
}

#[test]
fn batch_is_one_history_entry() {
    let mut document = opened_picture("batch.png", 2, &[RED, BLUE]);
    let bottom = bottom(&document);

    let changed = edit(
        &mut document,
        batch(
            "Brighten the corner",
            vec![
                invert(bottom),
                select_first_pixel(),
                Edit::Fill {
                    layer: bottom,
                    color: Color::WHITE,
                },
            ],
        ),
    );

    assert_eq!(changed.pixels, PixelChange::Content);
    assert!(changed.selection && changed.history);
    assert_eq!(whole_pixels(&document), [WHITE, YELLOW].concat());
    assert_eq!(
        document.history().actions(),
        [Action::Open, Action::Batch("Brighten the corner".into())]
    );

    let undone = edit(&mut document, Edit::Undo);
    assert!(undone.selection);
    assert_eq!(whole_pixels(&document), [RED, BLUE].concat());
    assert!(document.selection().is_empty());

    edit(&mut document, Edit::Redo);
    assert_eq!(whole_pixels(&document), [WHITE, YELLOW].concat());
    assert!(!document.selection().is_empty());
}

#[test]
fn failing_step_rolls_back_batch() {
    let mut document = opened_picture("batch-rollback.png", 2, &[RED, BLUE]);
    let bottom = bottom(&document);
    let foreign = opened_psd("batch-foreign.psd")
        .layers()
        .iter()
        .map(|layer| layer.id)
        .max()
        .expect("layers");

    let refused = document.apply(&batch(
        "Half done",
        vec![invert(bottom), select_first_pixel(), invert(foreign)],
    ));

    assert!(refused.is_err());
    assert_eq!(whole_pixels(&document), [RED, BLUE].concat());
    assert!(document.selection().is_empty());
    assert_eq!(document.history().actions(), [Action::Open]);
    assert!(!document.is_modified());
}

#[test]
fn psd_batch_undoes_all_layers() {
    let mut document = opened_psd("batch-layers.psd");
    let drawing = document.layers()[0].id;
    let paper = document.layers()[1].id;

    edit(
        &mut document,
        batch(
            "Paper only",
            vec![
                Edit::Show {
                    layer: drawing,
                    visible: false,
                },
                invert(paper),
            ],
        ),
    );
    assert_eq!(first_pixel(&document), YELLOW);
    assert_eq!(document.history().actions().len(), 2);

    edit(&mut document, Edit::Undo);
    assert_eq!(first_pixel(&document), RED);
    assert!(document.layers()[0].visible);
}
