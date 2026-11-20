use rikka_figure::{Lettering, Typeset};
use rikka_text::Affinity;
use rikka_types::Point;

const NEAR: f32 = 0.5;

#[test]
fn hits_place_the_caret_on_the_clicked_side() {
    let lettering = Lettering {
        text: "abc \u{5D0}\u{5D1}\u{5D2} def".into(),
        size: 40.0,
        origin: (10, 20),
        ..Lettering::default()
    };
    let typeset = Typeset::new(&lettering);
    let text = lettering.text.as_str();
    let mut boundary_sides = 0;
    for (at, _) in text.char_indices().skip(1) {
        let before = typeset.caret(at, Affinity::Before);
        let after = typeset.caret(at, Affinity::After);
        if (before.x() - after.x()).abs() > NEAR {
            boundary_sides += 1;
        }
        for caret in [before, after] {
            let hit = typeset.hit(Point::new(caret.x(), caret.y() + caret.height() / 2.0));
            let landed = typeset.caret(hit.at, hit.affinity);
            assert!(
                (landed.x() - caret.x()).abs() <= NEAR,
                "click at {} landed at {} for offset {at}",
                caret.x(),
                landed.x()
            );
        }
    }
    assert!(boundary_sides >= 2, "the text has no direction boundary");
}
