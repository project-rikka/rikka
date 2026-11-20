use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Rect, Size};
use rikka_ui::*;
use rikka_widgets::*;

use crate::common::dump_rect;

const VIEW: Size = Size::new(760.0, 300.0);
const NARROW: f32 = 240.0;
const LABEL: f32 = 12.0;
const GLYPH: f32 = 15.0;
const SAID: &str = "Review screenshot editor changes";
const SLACK: f32 = 1.0;

type Watch = Rc<Cell<Rect>>;

struct Watched {
    at: Watch,
}

impl Widget for Watched {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        self.at.set(ctx.bounds());
    }
}

fn watched(at: &Watch, child: impl View) -> Element {
    Element::with_children(Watched { at: at.clone() }, vec![child.build()])
}

#[derive(State)]
struct Chip {
    widest: f32,
    label_watch: Watch,
    stop: Watch,
    chip: Watch,
}

fn view(chip: &Chip) -> Element {
    let body = container(
        row! {
            icon(MaterialSymbol::HUB).extent(GLYPH);
            watched(
                &chip.label_watch,
                text(SAID).text_size(LABEL).no_wrap().grow(1.0),
            );
            watched(
                &chip.stop,
                button(icon(MaterialSymbol::STOP_CIRCLE).extent(GLYPH))
                    .icon_only()
                    .ghost()
                    .small()
                    .on_press(|| {}),
            );
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .surface()
    .border(1.0, palette().outline)
    .padding(Inset::xy(10.0, 4.0))
    .max_width(Length::px(chip.widest));

    row! { watched(&chip.chip, body) }.fill_width().build()
}

struct Shown {
    label_watch: Rect,
    stop: Rect,
    chip: Rect,
}

fn shown(widest: f32) -> Shown {
    let mut ui = TypedUi::new(
        Chip {
            widest,
            label_watch: Watch::default(),
            stop: Watch::default(),
            chip: Watch::default(),
        },
        view,
        Theme::dark(),
    );
    let mut scene = Scene::new();
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    let chip = ui.state();
    Shown {
        label_watch: chip.label_watch.get(),
        stop: chip.stop.get(),
        chip: chip.chip.get(),
    }
}

fn natural_width() -> f32 {
    let run = TextRun::new(SAID, LABEL).no_wrap();
    text_width(&run)
}

#[test]
fn narrow_row_elides_within_bounds() {
    let shown = shown(NARROW);

    assert!(
        shown.chip.width() <= NARROW,
        "chip exceeded given width: {:?}",
        shown.chip
    );
    assert!(
        shown.label_watch.max_x() <= shown.chip.max_x() + SLACK,
        "label outside chip: {:?} in {:?}",
        shown.label_watch,
        shown.chip
    );
    assert!(
        shown.stop.max_x() <= shown.chip.max_x() + SLACK,
        "button outside chip: {:?} in {:?}",
        shown.stop,
        shown.chip
    );
    assert!(
        shown.label_watch.width() < natural_width(),
        "label width exceeds chip"
    );
}

const SEGMENTS: [&str; 5] = ["Scene", "Lights", "Figures", "Overrides", "Effects"];

#[derive(State)]
struct Strip {
    room: f32,
}

fn strip(strip: &Strip) -> Element {
    column_of([container(
        segmented(0)
            .segments(SEGMENTS)
            .control_size(ControlSize::Small)
            .fill_width(),
    )
    .width(strip.room)
    .any()])
    .fill()
    .build()
}

fn segment_widths() -> Vec<f32> {
    let theme = Theme::dark();
    let size = ControlSize::Small;
    SEGMENTS
        .iter()
        .map(|label| {
            let run = TextRun::new(*label, size.font_size(&theme))
                .medium()
                .no_wrap();
            text_width(&run).ceil()
        })
        .collect()
}

struct Laid {
    dump: String,
    bar: Rect,
    natural: f32,
}

fn laid(room: f32) -> Laid {
    let mut ui = TypedUi::new(Strip { room }, strip, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut Scene::new());
    let dump = ui.dump();
    let line = dump
        .lines()
        .find(|line| line.trim_start().starts_with("Segmented "))
        .expect("segmented is laid out");
    let bar = dump_rect(line);
    let natural = line
        .split("content ")
        .nth(1)
        .and_then(|content| content.split('x').next())
        .and_then(|width| width.parse().ok())
        .expect("segmented reports its natural width");
    Laid { dump, bar, natural }
}

#[test]
fn segmented_shares_narrow_room_by_label_width() {
    let gap = ControlSize::Small.gap() * 2.0;
    let labels = segment_widths();
    let mut sorted = labels.clone();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let fitted: f32 = labels.iter().map(|label| label + gap).sum();
    let equal = (sorted[0] + gap) * labels.len() as f32;
    let natural = laid(VIEW.width).natural;
    let frame = natural - equal;
    let cases: [(f32, &[&str]); 3] = [
        (natural + 40.0, &[]),
        (fitted + frame, &[]),
        (
            fitted + frame - (sorted[0] - sorted[1]) / 2.0,
            &["Overrides"],
        ),
    ];
    for (room, expected) in cases {
        let Laid { dump, bar, .. } = laid(room);
        assert!(bar.width() <= room, "segmented {bar:?} overflows {room}");
        let shown: Vec<Rect> = dump
            .lines()
            .filter(|line| line.contains("· text"))
            .map(dump_rect)
            .collect();
        assert_eq!(
            shown.len(),
            SEGMENTS.len(),
            "every segment label is painted"
        );
        for rect in &shown {
            assert!(
                rect.x() >= bar.x() && rect.max_x() <= bar.max_x() + SLACK,
                "label {rect:?} outside {bar:?} at {room}"
            );
        }
        let elided: Vec<&str> = SEGMENTS
            .iter()
            .zip(shown.iter().zip(&labels))
            .filter(|(_, (rect, label))| rect.width() < **label)
            .map(|(segment, _)| *segment)
            .collect();
        assert_eq!(elided, expected, "elided labels at {room}");
    }
}

#[test]
fn elided_segment_shows_its_label_on_hover() {
    let gap = ControlSize::Small.gap() * 2.0;
    let labels = segment_widths();
    let mut sorted = labels.clone();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let fitted: f32 = labels.iter().map(|label| label + gap).sum();
    let natural = laid(VIEW.width).natural;
    let frame = natural - (sorted[0] + gap) * labels.len() as f32;
    let room = fitted + frame - (sorted[0] - sorted[1]) / 2.0;
    let Laid { dump, .. } = laid(room);
    let overrides = dump
        .lines()
        .filter(|line| line.contains("· text"))
        .map(dump_rect)
        .nth(3)
        .expect("the elided segment is painted");

    let mut ui = TypedUi::new(Strip { room }, strip, Theme::dark());
    let mut scene = Scene::new();
    let start = Instant::now();
    ui.set_viewport(VIEW, 1.0);
    ui.render(Rendering::new(&mut scene).with_time(start));
    ui.handle_event(Event::PointerMoved(overrides.center()));
    for millis in [600, 700, 900, 1300] {
        ui.render(Rendering::new(&mut scene).with_time(start + Duration::from_millis(millis)));
    }

    let quoted = format!("{:?}", "Overrides");
    assert!(
        ui.dump()
            .lines()
            .any(|line| line.contains("· text") && line.trim_end().ends_with(&quoted)),
        "hovered elided segment shows no tooltip"
    );
}
