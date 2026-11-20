use desk_layout::{Area, Content, Layout, Photos, Spot, Widget, WidgetId};
use rikka_types::{Point, Rect};

#[test]
fn widget_at_picks_topmost_under_point() {
    let area = Area::for_desk(Rect::new(0.0, 40.0, 1920.0, 1040.0));
    let layout = Layout {
        widgets: [1, 2]
            .map(|id| {
                Widget::new(
                    WidgetId(id),
                    Content::Photos(Photos::default()),
                    Spot::CENTER,
                )
            })
            .to_vec(),
    };
    let centre = area.room().center();
    let hit = layout.widget_at(&area, centre).map(|(widget, _)| widget);
    assert_eq!(hit, Some(WidgetId(2)));
    assert_eq!(layout.widget_at(&area, Point::new(1.0, 41.0)), None);
}
