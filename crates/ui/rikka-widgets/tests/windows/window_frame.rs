use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{CursorShape, Modifiers, Point, PointerButton, ResizeEdge, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

use crate::common::{self, UNCOVERED, renderer};

const VIEW: Size = Size::new(400.0, 300.0);

const BUTTON: f32 = 46.0;
const CAPTION_Y: f32 = 16.0;

fn caption_button(index: f32) -> Point {
    Point::new(VIEW.width - BUTTON * (index + 0.5), CAPTION_Y)
}

#[derive(State)]
struct Frame {
    chrome: WindowChrome,
    pressed: usize,
}

fn framed(frame: &Frame) -> impl View {
    let content = column! {
        button("保存").on_press(|frame: &mut Frame| frame.pressed += 1).fill()
    }
    .fill();
    window_frame(content).chrome(frame.chrome.clone())
}

fn frame(chrome: WindowChrome) -> (TypedUi<Frame>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Frame { chrome, pressed: 0 }, framed, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    (ui, scene)
}

#[derive(State)]
struct Captioned {
    chrome: WindowChrome,
    settings: bool,
}

fn captioned(captioned: &Captioned) -> impl View {
    let caption = match captioned.settings {
        true => text("設定").build(),
        false => button("設定")
            .small()
            .on_press(|captioned: &mut Captioned| captioned.settings = true)
            .build(),
    };
    window_frame(column_of(None::<Element>).fill())
        .chrome(captioned.chrome.clone())
        .caption(caption)
}

fn press(ui: &mut Ui, at: Point) {
    ui.handle_event(Event::PointerMoved(at));
    ui.handle_event(Event::PointerPressed {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
}

fn release(ui: &mut Ui, at: Point) {
    ui.handle_event(Event::PointerReleased {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
}

fn click(ui: &mut Ui, at: Point) {
    press(ui, at);
    release(ui, at);
}

#[test]
fn caption_buttons_send_window_requests() {
    let (mut ui, ..) = frame(WindowChrome::new("設定"));

    click(&mut ui, caption_button(0.0));
    assert_eq!(ui.take_window_requests(), [WindowRequest::AskToClose]);

    click(&mut ui, caption_button(1.0));
    assert_eq!(ui.take_window_requests(), [WindowRequest::ToggleMaximize]);

    click(&mut ui, caption_button(2.0));
    assert_eq!(ui.take_window_requests(), [WindowRequest::Minimize]);
}

#[test]
fn window_parts_name_caption_buttons_borders_and_content() {
    let (ui, ..) = frame(WindowChrome::new("設定"));
    for (at, part) in [
        (caption_button(0.0), Some(WindowPart::Close)),
        (caption_button(1.0), Some(WindowPart::Maximize)),
        (caption_button(2.0), Some(WindowPart::Minimize)),
        (Point::new(200.0, CAPTION_Y), Some(WindowPart::Caption)),
        (
            Point::new(2.0, 150.0),
            Some(WindowPart::Resize(ResizeEdge::Left)),
        ),
        (Point::new(200.0, 150.0), None),
    ] {
        assert_eq!(ui.window_part_at(at), part, "at {at:?}");
    }
}

#[test]
fn caption_double_click_ignores_press_on_caption_control() {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(
        Captioned {
            chrome: WindowChrome::new("設定"),
            settings: false,
        },
        captioned,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    let at = Point::new(20.0, CAPTION_Y);

    click(&mut ui, at);
    ui.render(&mut scene);
    assert!(ui.state().settings);
    click(&mut ui, at);
    assert!(ui.take_window_requests().is_empty());

    click(&mut ui, at);
    assert_eq!(ui.take_window_requests(), [WindowRequest::ToggleMaximize]);
}

#[test]
fn border_sets_resize_cursor_and_starts_resize() {
    let (mut ui, ..) = frame(WindowChrome::new("設定"));

    ui.handle_event(Event::PointerMoved(Point::new(2.0, 150.0)));
    assert_eq!(ui.cursor(), CursorShape::Resize(ResizeEdge::Left));

    ui.handle_event(Event::PointerPressed {
        position: Point::new(2.0, 150.0),
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    assert_eq!(
        ui.take_window_requests(),
        [WindowRequest::Resize(ResizeEdge::Left)]
    );

    ui.handle_event(Event::PointerMoved(Point::new(200.0, 150.0)));
    assert_eq!(ui.cursor(), CursorShape::Default);
    ui.handle_event(Event::PointerLeft);
    assert_eq!(ui.cursor(), CursorShape::Default);
}

#[test]
fn content_receives_its_clicks() {
    let (mut ui, mut scene) = frame(WindowChrome::new("設定"));

    click(&mut ui, Point::new(200.0, 150.0));
    ui.render(&mut scene);
    assert_eq!(ui.state().pressed, 1);
    assert!(ui.take_window_requests().is_empty());
}

#[test]
fn frame_fills_its_edges_and_corners() {
    let (width, height) = (VIEW.width as usize, VIEW.height as usize);
    let probes = [
        (0, height / 2),
        (width - 1, height / 2),
        (width / 2, height - 1),
        (0, height - 1),
        (width - 1, height - 1),
    ];
    let mut renderer = renderer();
    let mut target = renderer.create_target(width as u32, height as u32).unwrap();

    for chrome in [
        WindowChrome::new("設定"),
        WindowChrome::new("設定").maximized(true),
        WindowChrome::new("設定").fullscreen(true),
    ] {
        let mut scene = Scene::new();
        let mut ui = TypedUi::new(
            Frame {
                chrome: chrome.clone(),
                pressed: 0,
            },
            |frame: &Frame| {
                window_frame(column_of(None::<Element>).fill()).chrome(frame.chrome.clone())
            },
            Theme::dark(),
        );
        ui.set_background(Some(UNCOVERED));
        ui.set_viewport(VIEW, 1.0);
        let start = Instant::now();
        ui.render(Rendering::new(&mut scene).with_time(start));
        ui.render(Rendering::new(&mut scene).with_time(start + Duration::from_secs(2)));
        renderer.draw(&mut scene, &mut target).unwrap();
        let pixels = renderer.read_pixels(&mut target).unwrap();
        let inside = common::pixel(&pixels, width, width / 2, height / 2);
        for (x, y) in probes {
            assert_eq!(
                common::pixel(&pixels, width, x, y),
                inside,
                "{chrome:?}: ({x}, {y})"
            );
        }
    }
}
