use std::process::ExitCode;
use std::rc::Rc;
use std::sync::LazyLock;

use gpui::{
    App, AppContext, Bounds, Context, Entity, InputEvent, IntoElement, Pixels, Point, Render,
    ScrollDelta, ScrollEvent, TouchPhase, UniformListScrollHandle, Window, WindowBounds,
    WindowOptions, point, px, size,
};
use ui_scene::{Bounce, LIST_ROWS, Probe, Row, Scenario, Table, WINDOW_HEIGHT, WINDOW_WIDTH};

static PROBE: LazyLock<Probe> = LazyLock::new(Probe::from_env);

enum Content {
    Grid(ui_scene::Grid),
    List {
        rows: Rc<[Row]>,
        scroll: UniformListScrollHandle,
        bounce: Bounce,
    },
}

struct Root {
    content: Content,
}

impl Root {
    fn new(scenario: Scenario) -> Root {
        let content = match scenario.grid() {
            Some(grid) => Content::Grid(grid),
            None => Content::List {
                rows: Table::with_rows(LIST_ROWS).rows.into(),
                scroll: UniformListScrollHandle::new(),
                bounce: Bounce::new(LIST_ROWS),
            },
        };
        Root { content }
    }

    fn step(&mut self, cx: &mut Context<Root>) -> Option<(Point<Pixels>, Pixels)> {
        match &mut self.content {
            Content::Grid(grid) => {
                grid.advance();
                cx.notify();
                None
            }
            Content::List { scroll, bounce, .. } => {
                let handle = scroll.0.borrow().base_handle.clone();
                let scrolled = -handle.offset().y;
                Some((handle.bounds().center(), px(bounce.advance()) - scrolled))
            }
        }
    }
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Root>) -> impl IntoElement {
        match &self.content {
            Content::Grid(grid) => ui_gpui_views::grid(grid).into_any_element(),
            Content::List { rows, scroll, .. } => {
                ui_gpui_views::list(Rc::clone(rows), scroll).into_any_element()
            }
        }
    }
}

fn drive(root: Entity<Root>, window: &mut Window) {
    window.on_next_frame(move |window, cx| {
        PROBE.frame();
        let scroll = root.update(cx, Root::step);
        if let Some((position, distance)) = scroll {
            window.dispatch_event(
                ScrollEvent {
                    position,
                    delta: ScrollDelta::Pixels(point(px(0.0), -distance)),
                    touch_phase: TouchPhase::Moved,
                    ..ScrollEvent::default()
                }
                .to_platform_input(),
                cx,
            );
        }
        drive(root, window);
    });
}

fn main() -> ExitCode {
    let scenario = match Scenario::from_args() {
        Ok(scenario) => scenario,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    LazyLock::force(&PROBE);
    gpui_platform::application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..WindowOptions::default()
        };
        let window = cx
            .open_window(options, |_, cx| cx.new(|_| Root::new(scenario)))
            .expect("the benchmark window opens");
        if scenario.animates() {
            window
                .update(cx, |_, window, cx| drive(cx.entity(), window))
                .expect("the benchmark window is open");
        }
        cx.activate(true);
    });
    ExitCode::SUCCESS
}
