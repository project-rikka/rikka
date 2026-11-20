use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Color, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};
use std::time::{Duration, Instant};

use crate::common;

const VIEW: Size = Size::new(400.0, 300.0);
const TRIGGER: &str = "trigger";

const LONG: usize = 40;

const SETTLED: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct Ground;

impl Widget for Ground {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::FILL)
            .placed_at(rikka_types::Point::ZERO)
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {}

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        if let Event::PointerPressed {
            button: PointerButton::Secondary,
            position,
            ..
        } = event
        {
            let at = *position;
            ctx.run(move |probe: &mut Probe| {
                probe.open = true;
                probe.right_presses.push(at);
            });
        }
    }
}

#[derive(State)]
struct Probe {
    open: bool,
    long: bool,
    opens: u32,
    dismissals: u32,
    chosen: Option<usize>,
    text: TextDocument,
    blurs: u32,
    right_presses: Vec<Point>,
}

impl Probe {
    fn new() -> Self {
        Self {
            open: false,
            long: false,
            opens: 0,
            dismissals: 0,
            chosen: None,
            text: TextDocument::new("rikka"),
            blurs: 0,
            right_presses: Vec::new(),
        }
    }

    fn open(&mut self) {
        self.open = true;
        self.opens += 1;
    }
}

fn probing(probe: &Probe) -> impl View {
    let mut stage = column! {
        Element::new(Ground);
        button(text("open")).on_press(Probe::open).build().keyed(TRIGGER);
        text_field(&probe.text)
            .on_change(|| {})
            .on_blur(|probe: &mut Probe| probe.blurs += 1)
            .width(Length::px(200.0))
    }
    .gap(Space::SNUG)
    .padding(Space::SNUG)
    .fill();

    if probe.open {
        let rows: Vec<MenuItem> = if probe.long {
            (0..LONG)
                .map(|index| menu_item(format!("row {index}")))
                .collect()
        } else {
            vec![menu_item("コピー"), menu_item("貼り付け")]
        };
        stage = stage.push(
            menu()
                .items(rows)
                .on_activate(|probe: &mut Probe, index: usize| {
                    probe.open = false;
                    probe.chosen = Some(index);
                })
                .on_dismiss(|probe: &mut Probe| {
                    probe.open = false;
                    probe.dismissals += 1;
                })
                .build()
                .anchored(Anchor::below(TRIGGER).offset(0.0, 4.0)),
        );
    }
    stage
}

fn probe() -> (TypedUi<Probe>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Probe::new(), probing, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    (ui, scene)
}

fn click<S: State>(ui: &mut TypedUi<S>, at: Point, scene: &mut Scene) {
    ui.handle_event(Event::PointerMoved(at));
    ui.handle_event(Event::PointerPressed {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.handle_event(Event::PointerReleased {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(scene);
}

fn trigger() -> Point {
    Point::new(40.0, 26.0)
}

#[test]
fn outside_press_dismisses_menu_only() {
    let (mut ui, mut scene) = probe();

    click(&mut ui, trigger(), &mut scene);
    let away = Point::new(VIEW.width - 8.0, VIEW.height - 8.0);
    click(&mut ui, away, &mut scene);

    assert_eq!(ui.state().dismissals, 1);
    assert_eq!(ui.state().chosen, None);
}

fn settle(ui: &mut TypedUi<Probe>, scene: &mut Scene) {
    let now = Instant::now();
    ui.render(Rendering::new(scene).with_time(now));
    ui.render(Rendering::new(scene).with_time(now + SETTLED));
}

fn wheel(ui: &mut TypedUi<Probe>, at: Point, down: f32) {
    ui.handle_event(Event::Scroll {
        source: ScrollSource::Wheel,
        position: at,
        delta: ScrollDelta::from_offset(Point::new(0.0, down)),
        modifiers: Modifiers::NONE,
    });
}

#[test]
fn long_menu_fits_window_and_scrolls() {
    let (mut ui, mut scene) = probe();
    ui.update(|probe| probe.long = true);
    click(&mut ui, trigger(), &mut scene);

    click(&mut ui, Point::new(40.0, VIEW.height - 2.0), &mut scene);
    assert_eq!(ui.state().chosen, None, "panel outside window");
    assert_eq!(ui.state().dismissals, 1);

    let low = Point::new(40.0, VIEW.height - 12.0);
    click(&mut ui, trigger(), &mut scene);
    click(&mut ui, low, &mut scene);
    let unscrolled = ui.state().chosen.expect("panel row");
    assert!(unscrolled < LONG);

    click(&mut ui, trigger(), &mut scene);
    wheel(&mut ui, low, 100.0);
    settle(&mut ui, &mut scene);
    click(&mut ui, low, &mut scene);

    let scrolled = ui.state().chosen.expect("panel row");
    assert!(scrolled > unscrolled, "wheel scroll did not move rows");
    assert!(scrolled < LONG);
}

#[derive(State, Default)]
struct Roster {
    pressed: u32,
    dismissed: u32,
}

const ROW: f32 = 40.0;
const AIMED_ROW: usize = 2;

fn roster(_: &Roster) -> impl View {
    column_of([
        lazy_column(10, |slot| {
            text(format!("row {}", slot.index))
                .height(ROW)
                .fill_width()
                .build()
                .keyed(("row", slot.index))
        })
        .fill()
        .build(),
        popover(
            container(spacer())
                .on_press(|roster: &mut Roster| roster.pressed += 1)
                .size(Length::px(80.0), Length::px(30.0)),
        )
        .on_dismiss(|roster: &mut Roster| roster.dismissed += 1)
        .build()
        .anchored(Anchor::below(("row", AIMED_ROW))),
    ])
    .fill()
}

#[test]
fn overlay_anchors_to_lazy_row() {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Roster::default(), roster, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);

    let below_row = (AIMED_ROW + 1) as f32 * ROW;
    click(&mut ui, Point::new(30.0, below_row + 25.0), &mut scene);

    assert_eq!(ui.state().dismissed, 0, "overlay not anchored to row");
    assert_eq!(ui.state().pressed, 1);
}

const STAGE: &str = "stage";
const BLOCK: Color = Color::rgb(0.0, 0.0, 1.0);
const FRUITS: [&str; 5] = ["Apple", "Banana", "Blueberry", "Cherry", "Damson"];
const COMBO_AT: Point = Point::new(40.0, 24.0);
const PANEL_PROBE: (usize, usize) = (90, 120);
const SPREAD: u8 = 24;

#[derive(Clone, Copy, Debug)]
enum Holder {
    Sheet,
    Faded,
}

#[derive(State)]
struct Covered {
    holder: Holder,
    chosen: Option<usize>,
}

fn covered(covered: &Covered) -> Element {
    let picker = combo(covered.chosen)
        .options(FRUITS)
        .on_select(|covered: &mut Covered, row: usize| covered.chosen = Some(row))
        .width(160.0);
    let block = container(spacer())
        .background(BLOCK)
        .size(Length::FILL, Length::px(200.0));
    match covered.holder {
        Holder::Sheet => column_of([
            spacer().fill().build().keyed(STAGE),
            column! {
                picker;
                block;
            }
            .padding(10.0)
            .build()
            .anchored(Anchor::covering(STAGE)),
        ])
        .fill()
        .build(),
        Holder::Faded => column! {
            column! {
                picker;
                text("Fruit");
            }
            .opacity(0.5);
            block;
        }
        .padding(10.0)
        .fill()
        .build(),
    }
}

#[test]
fn open_combo_paints_above_later_content() {
    let mut renderer = common::renderer();
    let mut target = renderer
        .create_target(VIEW.width as u32, VIEW.height as u32)
        .unwrap();
    for holder in [Holder::Faded, Holder::Sheet] {
        let mut scene = Scene::new();
        let mut ui = TypedUi::new(
            Covered {
                holder,
                chosen: Some(0),
            },
            covered,
            Theme::dark(),
        );
        ui.set_viewport(VIEW, 1.0);
        ui.render(&mut scene);
        click(&mut ui, COMBO_AT, &mut scene);
        let now = Instant::now();
        ui.render(Rendering::new(&mut scene).with_time(now + SETTLED));
        renderer.draw(&mut scene, &mut target).unwrap();
        let pixels = renderer.read_pixels(&mut target).unwrap();

        let (x, y) = PANEL_PROBE;
        let (red, green, blue, _) = common::pixel(&pixels, VIEW.width as usize, x, y);
        let block = red < SPREAD && green < SPREAD && blue > u8::MAX - SPREAD;
        assert!(
            !block,
            "{holder:?}: the panel is under later content: {:?}",
            (red, green, blue)
        );
    }
}

const CLAIMED_BELOW: f32 = 60.0;
const POPOVER_TARGET: &str = "popover target";

#[derive(State, Default)]
struct Watched {
    claims: u32,
    chosen: Option<usize>,
    open: bool,
    pressed: u32,
    pointer: Option<Point>,
}

struct Watcher;

impl Widget for Watcher {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn behavior(&self, _state: &()) -> Behavior {
        Behavior::INTERCEPTS
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } if position.y > CLAIMED_BELOW => {
                ctx.capture_pointer();
                ctx.run(|watched: &mut Watched| watched.claims += 1);
            }
            Event::PointerReleased { .. } if ctx.captured() => ctx.release_pointer(),
            Event::PointerMoved(position) => {
                let at = *position;
                ctx.run(move |watched: &mut Watched| watched.pointer = Some(at));
            }
            _ => {}
        }
    }
}

fn watched(watched: &Watched) -> Element {
    let mut stage = column! {
        combo(watched.chosen)
            .options(FRUITS)
            .on_select(|watched: &mut Watched, row: usize| watched.chosen = Some(row))
            .width(160.0);
        spacer().size(Length::px(100.0), Length::px(20.0)).build().keyed(POPOVER_TARGET);
    }
    .padding(10.0)
    .fill();
    if watched.open {
        stage = stage.push(
            popover(
                container(spacer())
                    .on_press(|watched: &mut Watched| watched.pressed += 1)
                    .size(Length::px(200.0), Length::px(100.0)),
            )
            .build()
            .anchored(Anchor::below(POPOVER_TARGET)),
        );
    }
    Element::with_children(Watcher, vec![stage.build()])
}

#[test]
fn intercepting_ancestors_leave_open_overlays_alone() {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Watched::default(), watched, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    click(&mut ui, Point::new(300.0, 250.0), &mut scene);
    assert_eq!(ui.state().claims, 1, "bare presses reach the interceptor");

    click(&mut ui, COMBO_AT, &mut scene);
    ui.render(Rendering::new(&mut scene).with_time(Instant::now() + SETTLED));
    let row = Point::new(PANEL_PROBE.0 as f32, PANEL_PROBE.1 as f32);
    click(&mut ui, row, &mut scene);
    assert!(
        ui.state().chosen.is_some(),
        "the open combo panel takes its row"
    );
    assert_eq!(
        ui.state().pointer,
        Some(row),
        "the interceptor learns where the pointer is once the panel closes"
    );

    ui.update(|watched| watched.open = true);
    ui.render(Rendering::new(&mut scene).with_time(Instant::now() + SETTLED));
    click(&mut ui, Point::new(100.0, 120.0), &mut scene);
    assert_eq!(ui.state().pressed, 1, "the popover takes its press");
    assert_eq!(ui.state().claims, 1, "open overlays are never intercepted");
}
