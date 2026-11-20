use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(480.0, 200.0);

const SETTLED: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq, Eq)]
struct Live(String);

#[derive(State)]
struct Window {
    open: Vec<String>,
    torn_off: Vec<String>,
    beside_others: bool,
}

impl Window {
    fn holding(names: &[&str]) -> Self {
        Self {
            open: names.iter().map(|name| (*name).to_owned()).collect(),
            torn_off: Vec::new(),
            beside_others: false,
        }
    }

    fn names(&self) -> Vec<&str> {
        self.open.iter().map(String::as_str).collect()
    }

    fn lift(&mut self, at: usize) -> Option<Live> {
        (at < self.open.len()).then(|| Live(self.open.remove(at)))
    }

    fn land(&mut self, at: usize, Live(name): Live) {
        self.open.insert(at.min(self.open.len()), name);
    }

    fn tear_off(&mut self, Live(name): Live) {
        self.torn_off.push(name);
    }
}

fn strip(window: &Window) -> impl View {
    tabs(0)
        .items(window.open.iter().map(|name| tab(name.clone())))
        .on_select(|_: usize| {})
        .transfers(
            Transfer::group("rikka.test.tab")
                .take(Window::lift)
                .insert(Window::land)
                .tear_off(Window::tear_off, window.beside_others),
        )
}

fn window(names: &[&str]) -> (TypedUi<Window>, Scene) {
    made(Window::holding(names))
}

fn made(state: Window) -> (TypedUi<Window>, Scene) {
    let mut ui = TypedUi::new(state, strip, Theme::dark());
    let mut scene = Scene::new();
    ui.set_viewport(VIEW, 1.0);
    settle(&mut ui, &mut scene);
    (ui, scene)
}

fn settle(ui: &mut TypedUi<Window>, scene: &mut Scene) {
    let now = Instant::now();
    ui.render(Rendering::new(scene).with_time(now));
    ui.render(Rendering::new(scene).with_time(now + SETTLED));
}

fn width(slots: usize) -> f32 {
    ((VIEW.width - 8.0) / slots as f32).min(240.0)
}

fn tab_at(index: usize, slots: usize) -> Point {
    Point::new(4.0 + width(slots) * (index as f32 + 0.5), 22.0)
}

fn between(index: usize, slots: usize) -> Point {
    Point::new(4.0 + width(slots) * index as f32, 22.0)
}

fn offered(drag: &DragStart) -> String {
    drag.offer
        .mime_types()
        .next()
        .expect("tab drag offers tab mime type")
        .to_owned()
}

fn lift(ui: &mut TypedUi<Window>, scene: &mut Scene, index: usize, count: usize) -> DragStart {
    let pressed_at = tab_at(index, count);
    ui.handle_event(Event::PointerPressed {
        position: pressed_at,
        button: PointerButton::Primary,
        modifiers: Modifiers::default(),
    });
    ui.handle_event(Event::PointerMoved(Point::new(
        pressed_at.x,
        pressed_at.y + 24.0,
    )));
    settle(ui, scene);
    ui.take_drag_start().expect("pressed tab not dragged")
}

fn hover(ui: &mut TypedUi<Window>, scene: &mut Scene, at: Point, mime: &str) -> Option<String> {
    ui.handle_event(Event::DragMoved {
        position: at,
        offer: DragOffer::new([mime.to_owned()]),
    });
    settle(ui, scene);
    ui.drag_accepts().map(|accept| accept.mime.clone())
}

fn drop_at(ui: &mut TypedUi<Window>, scene: &mut Scene, at: Point, mime: &str) {
    ui.handle_event(Event::DragDropped {
        position: at,
        offer: DragOffer::new([mime.to_owned()]),
        action: DragAction::Move,
    });
    settle(ui, scene);
}

#[test]
fn tab_moves_between_windows() {
    let (mut origin, mut origin_scene) = window(&["one", "two"]);
    let (mut destination, mut destination_scene) = window(&["far"]);
    let drag = lift(&mut origin, &mut origin_scene, 1, 2);
    let landing = between(0, 2);

    let accepted = hover(
        &mut destination,
        &mut destination_scene,
        landing,
        &offered(&drag),
    );
    assert_eq!(accepted.as_deref(), Some(offered(&drag).as_str()));

    origin.handle_event(Event::DragTaken);
    drop_at(
        &mut destination,
        &mut destination_scene,
        landing,
        &offered(&drag),
    );
    origin.handle_event(Event::DragEnded(DragEnd::Landed(DragAction::Move)));
    settle(&mut origin, &mut origin_scene);

    assert_eq!(origin.state().names(), ["one"]);
    assert_eq!(destination.state().names(), ["two", "far"]);
}

#[test]
fn foreign_group_strip_rejects_drag() {
    let (mut ui, mut scene) = window(&["one", "two"]);

    let accepted = hover(
        &mut ui,
        &mut scene,
        tab_at(0, 2),
        "application/x-rikka-tab;group=elsewhere;pid=1",
    );

    assert_eq!(accepted, None);
}

#[test]
fn unclaimed_tab_drop_opens_window() {
    let (mut ui, mut scene) = window(&["one", "two"]);
    let _drag = lift(&mut ui, &mut scene, 0, 2);

    ui.handle_event(Event::DragEnded(DragEnd::Nowhere));
    settle(&mut ui, &mut scene);

    assert_eq!(ui.state().names(), ["two"]);
    assert_eq!(ui.state().torn_off, ["one"]);
}

#[test]
fn cancelled_tab_drag_returns_to_source() {
    let (mut ui, mut scene) = window(&["one", "two", "three"]);
    let _drag = lift(&mut ui, &mut scene, 1, 3);

    ui.handle_event(Event::DragTaken);
    ui.handle_event(Event::DragEnded(DragEnd::Landed(DragAction::Move)));
    settle(&mut ui, &mut scene);

    assert_eq!(ui.state().names(), ["one", "two", "three"]);
}
