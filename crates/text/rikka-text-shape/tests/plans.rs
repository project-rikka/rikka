use std::sync::Arc;

use rikka_text_font::{FontFile, Location};
use rikka_text_shape::{Direction, SegmentProperties, ShapeFace, ShapePlan};
use rikka_text_unicode::Script;

use rikka_test_fonts::loaded_font;

fn face_of(file_name: &str) -> Arc<ShapeFace> {
    let font = FontFile::new(loaded_font(file_name))
        .unwrap()
        .face(0)
        .unwrap();
    ShapeFace::new(font, Location::default())
}

fn plan(face: &Arc<ShapeFace>, properties: SegmentProperties) -> ShapePlan {
    face.plan(&properties, &[])
}

#[test]
fn direction_or_complex_shaper_breaks_equivalence() {
    let face = face_of("NotoSansArabic.ttf");
    let latin = SegmentProperties::new(Script::Latin);
    let arabic = plan(&face, SegmentProperties::new(Script::Arabic));
    let syriac = plan(&face, SegmentProperties::new(Script::Syriac));
    assert!(!arabic.is_equivalent(&syriac));
    assert!(arabic.is_equivalent(&arabic.clone()));
    assert!(
        !plan(&face, latin)
            .is_equivalent(&plan(&face, latin.with_direction(Direction::RightToLeft)))
    );
}

#[test]
fn plans_of_different_faces_differ() {
    let properties = SegmentProperties::new(Script::Latin);
    let first = plan(&face_of("NotoSans.ttf"), properties);
    let second = plan(&face_of("NotoSans.ttf"), properties);
    assert!(!first.is_equivalent(&second));
}
