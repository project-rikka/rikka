use desk_layout::{
    Clock, Content, Desks, Face, Kind, Meter, Monitor, Note, PhotoInterval, Photos, Size, Spot,
    Widget, WidgetId,
};

fn widget(id: u64, content: Content) -> Widget {
    Widget::new(WidgetId(id), content, Spot::new(0.25, 0.75))
}

#[test]
fn desk_round_trips() {
    let mut desks = Desks::default();
    let shown = desks.layout_mut(Some("sunset.png"));
    shown.widgets.push(widget(
        1,
        Content::Clock(Clock {
            face: Face::Analog,
            seconds: true,
            zone: Some("Asia/Tokyo".into()),
        }),
    ));
    shown.widgets.push(widget(
        2,
        Content::Note(Note {
            text: "milk\n\"eggs\" = 3".into(),
        }),
    ));
    shown.widgets.push(widget(
        3,
        Content::Monitor(Monitor {
            meters: vec![Meter::Gpu, Meter::Disk],
        }),
    ));
    let mut photos = widget(4, Content::Photos(Photos::default()));
    photos.size = Size::ExtraLarge;
    desks.layout_mut(None).widgets.push(photos);

    let read = Desks::parse(&desks.document()).expect("its own document");
    assert!(read.skipped.is_empty(), "{:?}", read.skipped);
    assert_eq!(read.desks, desks);
}

#[test]
fn skips_unknown_or_broken_widgets() {
    let text = r#"
        [[desk]]
        wallpaper = "a.png"

        [[desk.widget]]
        id = 1
        size = "small"
        x = 0.1
        y = 0.2
        kind = "clock"

        [[desk.widget]]
        id = 2
        size = "small"
        x = 0.5
        y = 0.5
        kind = "hologram"

        [[desk.widget]]
        id = 1
        size = "small"
        x = 0.5
        y = 0.5
        kind = "note"

        [[desk.widget]]
        id = 3
        size = "extra-large"
        x = 7
        y = -1
        kind = "power"
    "#;
    let read = Desks::parse(text).expect("valid TOML");
    assert_eq!(read.skipped.len(), 2);
    let layout = read.desks.layout(Some("a.png")).expect("its layout");
    let kinds: Vec<Kind> = layout.widgets.iter().map(Widget::kind).collect();
    assert_eq!(kinds, [Kind::Clock, Kind::Power]);
    let power = &layout.widgets[1];
    assert_eq!(power.size, Kind::Power.default_size());
    assert_eq!(power.spot, Spot::new(1.0, 0.0));
}

#[test]
fn photo_interval_snaps_to_nearest() {
    let text = r#"
        [[desk]]
        wallpaper = "a.png"

        [[desk.widget]]
        id = 1
        size = "small"
        x = 0.1
        y = 0.2
        kind = "photos"
        interval = 100
    "#;
    let read = Desks::parse(text).expect("valid TOML");
    let layout = read.desks.layout(Some("a.png")).expect("its layout");
    let Content::Photos(photos) = &layout.widgets[0].content else {
        panic!("not a photos widget: {:?}", layout.widgets[0].content);
    };
    assert_eq!(photos.interval, PhotoInterval::Minute);
    assert!(Desks::parse(&read.desks.document()).is_ok_and(|reread| reread.desks == read.desks));
}

#[test]
fn rejects_invalid_toml() {
    assert!(Desks::parse("[[desk]\nwallpaper = ").is_err());
}

#[test]
fn new_wallpaper_copies_previous_layout() {
    let mut desks = Desks::default();
    desks
        .layout_mut(Some("a.png"))
        .widgets
        .push(widget(1, Content::new(Kind::Weather)));

    assert!(desks.arrive(Some("b.png"), Some("a.png")));
    assert_eq!(desks.layout(Some("b.png")), desks.layout(Some("a.png")));

    desks.layout_mut(Some("a.png")).widgets.clear();
    assert_eq!(
        desks.layout(Some("b.png")).map(|l| l.widgets.len()),
        Some(1)
    );

    assert!(!desks.arrive(Some("b.png"), Some("a.png")));
    assert_eq!(
        desks.layout(Some("b.png")).map(|l| l.widgets.len()),
        Some(1)
    );

    assert!(desks.arrive(Some("c.png"), Some("unknown.png")));
    assert_eq!(
        desks.layout(Some("c.png")).map(|l| l.widgets.len()),
        Some(0)
    );
    assert_eq!(desks.fresh_id(), WidgetId(2));
}
