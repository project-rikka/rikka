use rikka_image_edit::{Invert, Mask, Operation, Recipe, Step};

use crate::support::*;
use paint_document::{Action, Area, PixelChange, Saving};

fn inverted(document: &mut Document, layer: LayerId) -> Change {
    develop(document, layer, Recipe::new().then(Invert))
}

#[test]
fn undo_redo_restores_developed_picture() {
    let mut document = opened_picture("undo-develop.png", 2, &[RED, BLUE]);
    let bottom = bottom(&document);
    inverted(&mut document, bottom);
    let developed = whole_pixels(&document);

    let undone = edit(&mut document, Edit::Undo);
    assert_eq!(whole_pixels(&document), [RED, BLUE].concat());
    assert_eq!(undone.pixels, PixelChange::Content);
    assert!(undone.history);

    edit(&mut document, Edit::Redo);
    assert_eq!(whole_pixels(&document), developed);
    assert_eq!(
        document.history().actions(),
        [Action::Open, Action::Develop(Operation::Invert(Invert))]
    );
    assert!(edit(&mut document, Edit::Redo).is_none());
}

#[test]
fn undo_redo_restores_patched_pixels() {
    let shades: Vec<[u8; 4]> = (0..12u32 * 9)
        .map(|index| [(index * 7 % 256) as u8, (index * 3 % 256) as u8, 90, 255])
        .collect();
    let mut document = opened_picture("undo-patch.png", 12, &shades);
    let bottom = bottom(&document);
    let before = whole_pixels(&document);
    let corner = Step::new(Invert).masked(Mask::rectangle(7.0, 5.0, 2.0, 3.0));
    develop(&mut document, bottom, Recipe::new().then(corner));
    let developed = whole_pixels(&document);
    assert_ne!(developed, before);

    for _ in 0..2 {
        edit(&mut document, Edit::Undo);
        assert_eq!(whole_pixels(&document), before);
        edit(&mut document, Edit::Redo);
        assert_eq!(whole_pixels(&document), developed);
    }
}

#[test]
fn revisit_jumps_to_any_state() {
    let mut document = opened_picture("revisit.png", 2, &[RED; 4]);
    select(&mut document, Region::rectangle(Area::new(0, 0, 1, 1)));
    let bottom = bottom(&document);
    inverted(&mut document, bottom);
    select(&mut document, Region::rectangle(Area::new(1, 1, 1, 1)));

    let back = edit(&mut document, Edit::Revisit(0));
    assert_eq!(back.pixels, PixelChange::Content);
    assert!(back.selection);
    assert_eq!(document.history().current(), 0);
    assert_eq!(first_pixel(&document), RED);
    assert!(document.selection().is_empty());

    edit(&mut document, Edit::Revisit(2));
    assert_eq!(document.history().current(), 2);
    assert_eq!(first_pixel(&document), [0, 255, 255, 255]);
    assert_eq!(document.selection().bounds(), Some(Area::new(0, 0, 1, 1)));
    assert!(edit(&mut document, Edit::Revisit(2)).is_none());
}

#[test]
fn new_edit_discards_redo_states() {
    let mut document = opened_picture("branch.png", 2, &[RED; 4]);
    let bottom = bottom(&document);
    inverted(&mut document, bottom);
    select(&mut document, Region::rectangle(Area::new(0, 0, 1, 1)));
    edit(&mut document, Edit::Undo);
    edit(&mut document, Edit::Undo);

    select(&mut document, Region::rectangle(Area::new(1, 0, 1, 1)));

    let history = document.history();
    assert_eq!(
        history.actions(),
        [Action::Open, Action::Select(SelectionAction::Rectangle)]
    );
    assert!(!history.can_redo());
    assert_eq!(first_pixel(&document), RED);
}

#[test]
fn modified_until_saved_state() {
    let mut document = opened_picture("modified.png", 2, &[RED; 4]);
    assert!(!document.is_modified());

    let bottom = bottom(&document);
    inverted(&mut document, bottom);
    assert!(document.is_modified());
    edit(&mut document, Edit::Undo);
    assert!(!document.is_modified());
    edit(&mut document, Edit::Redo);
    let path = folder("modified").join("saved.png");
    document.save(&path, &Saving::default()).expect("a save");
    assert!(!document.is_modified());
    assert_eq!(document.history().saved(), Some(1));

    edit(&mut document, Edit::Undo);
    assert!(document.is_modified());
    edit(&mut document, Edit::Redo);
    assert!(!document.is_modified());
}
