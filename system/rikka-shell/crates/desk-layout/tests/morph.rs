use desk_layout::{Content, Kind, Spot, Widget, WidgetId, WidgetMatch, pair};

fn at(id: u64, kind: Kind, x: f32) -> Widget {
    Widget::new(WidgetId(id), Content::new(kind), Spot::new(x, 0.5))
}

#[test]
fn widgets_pair_by_id_then_kind() {
    let before = [
        at(1, Kind::Clock, 0.1),
        at(2, Kind::Weather, 0.9),
        at(3, Kind::Note, 0.5),
        at(4, Kind::Weather, 0.2),
    ];
    let after = [
        at(9, Kind::Weather, 0.15),
        at(1, Kind::Clock, 0.8),
        at(7, Kind::Media, 0.5),
        at(8, Kind::Weather, 0.95),
    ];
    assert_eq!(
        pair(&before, &after),
        WidgetMatch {
            matched: vec![(3, 0), (0, 1), (1, 3)],
            removed: vec![2],
            added: vec![2],
        }
    );
}
