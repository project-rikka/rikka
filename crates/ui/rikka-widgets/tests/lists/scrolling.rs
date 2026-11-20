use rikka_types::{Modifiers, PointerButton};
use std::time::{Duration, Instant};

use rikka_renderer::{OffscreenTarget, Preparer, Renderer, Scene, SceneFrame};
use rikka_test_renderer::TestRenderer;
use rikka_theme::Theme;
use rikka_types::{Color, Point, Size};
use rikka_ui::scroll::ScrollPosition;
use rikka_ui::*;
use rikka_widgets::{column, *};

use crate::common::{UNCOVERED, renderer};

const VIEW: Size = Size::new(600.0, 300.0);

const MIDDLE: Point = Point::new(VIEW.width / 2.0, VIEW.height / 2.0);

type Shape = fn() -> Element;

#[derive(State)]
struct Page {
    shape: Shape,
}

fn cards() -> Flex {
    let cards = (0..8).map(|i| {
        container(text(format!("card {i}")).title())
            .surface()
            .padding(Space::LOOSE)
    });
    column_of(cards).gap(Space::ROOMY).padding(Space::LOOSE)
}

fn header() -> Element {
    container(text("header")).padding(Space::ROOMY).build()
}

fn sidebar() -> Element {
    container(spacer()).width(Length::px(200.0)).build()
}

fn page_view(page: &Page) -> impl View {
    (page.shape)()
}

fn root() -> Element {
    scrollable(cards()).fill().build()
}

fn column_grow() -> Element {
    column! {
        header();
        scrollable(cards()).grow(1.0)
    }
    .fill()
    .build()
}

fn row_grow() -> Element {
    row! {
        sidebar();
        scrollable(cards()).grow(1.0)
    }
    .fill()
    .build()
}

fn column_row_grow() -> Element {
    column! {
        header();
        row! {
            sidebar();
            scrollable(cards()).grow(1.0)
        }
        .grow(1.0)
    }
    .fill()
    .build()
}

fn column_row_fill() -> Element {
    column! {
        header();
        row! {
            sidebar();
            scrollable(cards()).fill()
        }
        .grow(1.0)
    }
    .fill()
    .build()
}

fn scrolls(name: &str, shape: Shape) -> bool {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Page { shape }, page_view, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let settled = Instant::now() + Duration::from_secs(5);
    ui.render(Rendering::new(&mut scene).with_time(settled));
    ui.render(Rendering::new(&mut scene).with_time(settled));
    assert!(!ui.needs_render(), "{name}: never settled");

    ui.handle_event(Event::Scroll {
        source: ScrollSource::Wheel,
        position: Point::new(400.0, 150.0),
        delta: ScrollDelta::from_offset(Point::new(0.0, 60.0)),
        modifiers: Modifiers::NONE,
    });
    ui.needs_render()
}

#[test]
fn scrollable_scrolls_at_any_depth() {
    let shapes: [(&str, Shape); 5] = [
        ("root", root),
        ("column > grow", column_grow),
        ("row > grow", row_grow),
        ("column > row > grow", column_row_grow),
        ("column > row > fill", column_row_fill),
    ];
    let broken: Vec<&str> = shapes
        .into_iter()
        .filter(|(name, shape)| !scrolls(name, *shape))
        .map(|(name, _)| name)
        .collect();
    assert!(broken.is_empty(), "did not scroll: {broken:?}");
}

fn settle<S: State>(ui: &mut TypedUi<S>, from: Instant, scene: &mut Scene) -> Instant {
    let mut now = from;
    for _ in 0..16 {
        ui.render(Rendering::new(scene).with_time(now));
        if !ui.needs_render() {
            return now;
        }
        now += Duration::from_millis(120);
    }
    panic!("never settled");
}

type Watch = std::rc::Rc<std::cell::Cell<Rect>>;

struct Watched {
    at: Watch,
}

impl Widget for Watched {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().width(Length::FILL)
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_PLACE
    }

    fn paint(&self, _state: &mut Self::State, ctx: &mut PaintCtx<'_>) {
        self.at.set(ctx.bounds());
    }
}

const LANE_ROW: f32 = 60.0;

#[derive(State)]
struct Laned {
    rows: usize,
    virtualized: bool,
    padding: f32,
    at: Watch,
    pressed: bool,
}

impl Laned {
    fn row(&self, index: usize) -> Element {
        let row = container(text(format!("row {index}")))
            .height(Length::px(LANE_ROW))
            .fill_width()
            .on_press(|laned: &mut Laned| laned.pressed = true)
            .focusless();
        match index {
            0 => Element::with_children(
                Watched {
                    at: self.at.clone(),
                },
                vec![row.build()],
            ),
            _ => row.build(),
        }
    }
}

fn laned(laned: &Laned) -> impl View {
    if laned.virtualized {
        let (rows, at) = (laned.rows, laned.at.clone());
        return lazy_column(rows, move |slot| {
            let index = slot.index;
            let row = container(text(format!("row {index}")))
                .height(Length::px(LANE_ROW))
                .fill_width();
            match index {
                0 => Element::with_children(Watched { at: at.clone() }, vec![row.build()]),
                _ => row.build(),
            }
        })
        .uniform(Length::px(LANE_ROW))
        .fill()
        .build();
    }
    let rows: Vec<Element> = (0..laned.rows).map(|index| laned.row(index)).collect();
    scrollable(
        column_of(rows)
            .padding(Inset::all(laned.padding))
            .fill_width(),
    )
    .fill()
    .build()
}

struct Lane {
    ui: TypedUi<Laned>,
    scene: Scene,
    now: Instant,
}

impl Lane {
    fn showing(rows: usize, virtualized: bool) -> Self {
        Self::padded(rows, virtualized, 0.0)
    }

    fn padded(rows: usize, virtualized: bool, padding: f32) -> Self {
        let mut scene = Scene::new();
        let mut ui = TypedUi::new(
            Laned {
                rows,
                virtualized,
                padding,
                at: Watch::default(),
                pressed: false,
            },
            laned,
            Theme::dark(),
        );
        ui.set_viewport(VIEW, 1.0);
        let now = settle(&mut ui, Instant::now(), &mut scene);
        Lane { ui, scene, now }
    }

    fn row(&self) -> Rect {
        self.ui.state().at.get()
    }

    fn press(&mut self, at: Point) -> bool {
        for event in [
            Event::PointerMoved(at),
            Event::PointerPressed {
                position: at,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
            Event::PointerReleased {
                position: at,
                button: PointerButton::Primary,
                modifiers: Modifiers::NONE,
            },
        ] {
            self.ui.handle_event(event);
        }
        self.now = settle(&mut self.ui, self.now, &mut self.scene);
        self.ui.state().pressed
    }
}

#[test]
fn scrollbar_press_does_not_reach_row() {
    let mut scrolls = Lane::showing(40, false);
    let row = scrolls.row();

    assert!(scrolls.press(row.center()), "row did not take its press");

    let mut again = Lane::showing(40, false);
    let over_bar = Point::new(VIEW.width - 1.0, row.center().y);
    assert!(!again.press(over_bar), "scrollbar overlaps row: {row:?}");
}

const PAGE_ROWS: usize = 40;
const PAGE_ROW: f32 = 28.0;
const WHEEL: f32 = 400.0;

fn rows(at: &Watch, row: impl Fn(usize) -> Element) -> Element {
    Element::with_children(
        Watched { at: at.clone() },
        vec![column_of((0..PAGE_ROWS).map(row)).fill_width().build()],
    )
}

fn row(label: String) -> Container {
    container(text(label))
        .height(Length::px(PAGE_ROW))
        .fill_width()
}

#[derive(State)]
struct Tabs {
    tab: usize,
    places: [Place; 2],
    at: Watch,
}

impl Tabs {
    fn rows(&self) -> Element {
        rows(&self.at, |index| {
            row(format!("tab {} row {index}", self.tab)).build()
        })
    }
}

fn tabbed(tabs: &Tabs) -> impl View {
    let pane = scrollable(tabs.rows()).place(&tabs.places[tabs.tab]).fill();
    match tabs.tab {
        0 => column! {
            row("tabs".to_owned());
            pane.grow(1.0)
        }
        .fill()
        .build(),
        _ => pane.build(),
    }
}

#[test]
fn remounted_pane_restores_scroll() {
    let at = Watch::default();
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(
        Tabs {
            tab: 0,
            places: [Place::new(), Place::new()],
            at: at.clone(),
        },
        tabbed,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let mut now = settle(&mut ui, Instant::now(), &mut scene);

    ui.handle_event(Event::Scroll {
        source: ScrollSource::Wheel,
        position: MIDDLE,
        delta: ScrollDelta::from_offset(Point::new(0.0, WHEEL)),
        modifiers: Modifiers::NONE,
    });
    now = settle(&mut ui, now, &mut scene);
    let left = -at.get().y();
    assert!(left > 0.0, "first tab did not scroll");

    ui.update(|tabs| tabs.tab = 1);
    now = settle(&mut ui, now, &mut scene);
    assert_eq!(-at.get().y(), 0.0, "tabs share scroll position");

    ui.update(|tabs| tabs.tab = 0);
    now += Duration::from_millis(120);
    ui.render(Rendering::new(&mut scene).with_time(now));

    assert_eq!(-at.get().y(), left, "tab lost scroll position");
    assert!(
        !ui.needs_render(),
        "tab drawn at drop point instead of animated"
    );
}

const SECTIONS: usize = 40;

#[derive(State)]
struct Sections {
    chosen: Option<usize>,
}

fn sections(sections: &Sections) -> impl View {
    let nav = nav_list(sections.chosen.unwrap_or(0))
        .items((0..SECTIONS).map(|index| nav_item(format!("section {index}"))))
        .on_select(|sections: &mut Sections, index: usize| sections.chosen = Some(index));
    split_view(nav, text("body"))
}

#[test]
fn split_view_navigation_scrolls_to_its_last_item() {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Sections { chosen: None }, sections, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let mut now = settle(&mut ui, Instant::now(), &mut scene);

    let sidebar = Point::new(100.0, VIEW.height / 2.0);
    for _ in 0..8 {
        ui.handle_event(Event::Scroll {
            source: ScrollSource::Wheel,
            position: sidebar,
            delta: ScrollDelta::from_offset(Point::new(0.0, WHEEL)),
            modifiers: Modifiers::NONE,
        });
        now = settle(&mut ui, now, &mut scene);
    }

    let bottom = Point::new(100.0, VIEW.height - 20.0);
    for event in [
        Event::PointerMoved(bottom),
        Event::PointerPressed {
            position: bottom,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        },
        Event::PointerReleased {
            position: bottom,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        },
    ] {
        ui.handle_event(event);
    }
    assert_eq!(ui.state().chosen, Some(SECTIONS - 1));
}

const SHELF: Size = Size::new(320.0, 240.0);
const SHELF_ROWS: usize = 400;
const SHELF_ROW: f32 = 30.0;

#[derive(Clone, Copy, Debug)]
enum Shelf {
    Lazy,
    Eager,
}

#[derive(State)]
struct Shelved {
    #[untracked]
    shelf: Shelf,
    #[untracked]
    place: Place,
}

fn shelf_row(index: usize) -> Element {
    let shade = match index % 3 {
        0 => Color::rgb(0.18, 0.2, 0.26),
        1 => Color::rgb(0.24, 0.16, 0.2),
        _ => Color::rgb(0.14, 0.22, 0.16),
    };
    container(
        row! {
            icon(MaterialSymbol::FOLDER).extent(Length::px(16.0));
            text(format!("row {index} with some words")).grow(1.0);
            text(format!("{}", index * 7)).caption()
        }
        .gap(6.0)
        .align_items(Align::Center),
    )
    .background(shade)
    .radius(6.0)
    .padding(Inset::exact(4.0, 8.0, 4.0, 8.0))
    .height(Length::px(SHELF_ROW))
    .fill_width()
    .build()
}

fn shelved(shelved: &Shelved) -> Element {
    match shelved.shelf {
        Shelf::Lazy => lazy_column(SHELF_ROWS, |slot| shelf_row(slot.index))
            .uniform(Length::px(SHELF_ROW))
            .place(&shelved.place)
            .fill()
            .build(),
        Shelf::Eager => scrollable(column_of((0..SHELF_ROWS).map(shelf_row)).fill_width())
            .place(&shelved.place)
            .fill()
            .build(),
    }
}

fn stand(shelf: Shelf, offset: f32) -> ScrollPosition {
    ScrollPosition {
        subject: match shelf {
            Shelf::Lazy => Some(0),
            Shelf::Eager => None,
        },
        offset,
        ..ScrollPosition::default()
    }
}

fn shelved_ui(shelf: Shelf, scale: f32) -> TypedUi<Shelved> {
    let mut ui = TypedUi::new(
        Shelved {
            shelf,
            place: Place::new(),
        },
        shelved,
        Theme::dark(),
    );
    ui.set_background(Some(UNCOVERED));
    ui.set_viewport(SHELF, scale);
    ui
}

struct Shelving {
    renderer: TestRenderer,
    target: OffscreenTarget,
    scene: Scene,
    mirror: Scene,
    frame: SceneFrame,
    preparer: Preparer,
    ui: TypedUi<Shelved>,
    now: Instant,
}

impl Shelving {
    fn open(shelf: Shelf, scale: f32) -> Shelving {
        let mut renderer = renderer();
        let (width, height) = shelf_pixels(scale);
        let target = renderer.create_target(width, height).unwrap();
        let preparer = renderer.preparer();
        let mut shelving = Shelving {
            renderer,
            target,
            scene: Scene::new(),
            mirror: Scene::new(),
            frame: SceneFrame::default(),
            preparer,
            ui: shelved_ui(shelf, scale),
            now: Instant::now(),
        };
        shelving.settle();
        shelving
    }

    fn settle(&mut self) -> Vec<u8> {
        for _ in 0..240 {
            self.now += Duration::from_millis(16);
            if self.ui.needs_render() {
                self.ui
                    .render(Rendering::new(&mut self.scene).with_time(self.now));
            }
            self.scene.take_frame(&mut self.frame);
            self.mirror.apply(&mut self.frame);
            self.preparer.prepare(&mut self.mirror).unwrap();
            self.renderer
                .draw(&mut self.mirror, &mut self.target)
                .unwrap();
            if !self.mirror.is_animating() && !self.ui.needs_render() {
                break;
            }
        }
        self.renderer.read_pixels(&mut self.target).unwrap()
    }
}

fn shelf_pixels(scale: f32) -> (u32, u32) {
    ((SHELF.width * scale) as u32, (SHELF.height * scale) as u32)
}

fn fresh_shelf(renderer: &mut Renderer, shelf: Shelf, scale: f32, offset: f32) -> Vec<u8> {
    let (width, height) = shelf_pixels(scale);
    let mut target = renderer.create_target(width, height).unwrap();
    let mut scene = Scene::new();
    let mut ui = shelved_ui(shelf, scale);
    ui.state().place.set(stand(shelf, offset));
    let start = Instant::now();
    let mut now = start;
    for _ in 0..240 {
        ui.render(Rendering::new(&mut scene).with_time(now));
        if !ui.needs_render() && !scene.is_animating() {
            break;
        }
        now += Duration::from_millis(16);
    }
    renderer.draw(&mut scene, &mut target).unwrap();
    renderer.read_pixels(&mut target).unwrap()
}

#[test]
fn scrolled_rows_draw_like_a_fresh_list() {
    let offsets = [
        1.0, 4.0, 17.0, 3.0, 67.0, 600.0, 2.0, 9_000.0, 7.0, 31.0, 0.0,
    ];
    for (shelf, scale) in [(Shelf::Lazy, 1.0), (Shelf::Eager, 1.0), (Shelf::Lazy, 2.0)] {
        let mut shelving = Shelving::open(shelf, scale);
        let mut offset = 0.0;
        for by in offsets {
            offset = match by {
                0.0 => 0.0,
                by => offset + by,
            };
            shelving.ui.state().place.set(stand(shelf, offset));
            let drawn = shelving.settle();
            let shown = shelving.ui.state().place.get().offset;
            assert_eq!(
                shown, offset,
                "{shelf:?} at {scale}x did not reach {offset}"
            );
            let expected = fresh_shelf(&mut shelving.renderer, shelf, scale, offset);
            let differing = drawn
                .chunks(4)
                .zip(expected.chunks(4))
                .filter(|(drawn, expected)| drawn != expected)
                .count();
            assert_eq!(
                differing, 0,
                "{shelf:?} at {scale}x scrolled to {offset}: {differing} pixels differ from a fresh draw"
            );
        }
    }
}
