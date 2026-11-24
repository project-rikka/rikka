use rikka_sync::{Mutex, MutexGuard};
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_terminal::{Config, Font, Options, Report, Session, screen};
use rikka_test_renderer::TestRenderer;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, ScrollDelta, Size};
use rikka_ui::{Event, Job, JobResult, Rendering, ScrollSource, State, Styled, TypedUi, Work};

const VIEW: Size = Size::new(640.0, 400.0);

const SETTLED: Duration = Duration::from_secs(2);

pub(crate) const TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(20);

static ALONE: Mutex<()> = Mutex::new(());

pub(crate) fn alone() -> MutexGuard<'static, ()> {
    ALONE.lock()
}

#[derive(State)]
struct Window {
    session: Rc<Session>,
    font: Font,
    active: bool,
}

fn open_ui(session: &Rc<Session>, view: Size) -> TypedUi<Window> {
    let mut ui = TypedUi::new(
        Window {
            session: Rc::clone(session),
            font: Config::load().font(),
            active: true,
        },
        |window: &Window| {
            screen(Rc::clone(&window.session), window.font.clone())
                .active(window.active)
                .grow(1.0)
                .fill_width()
        },
        Theme::dark(),
    );
    ui.set_viewport(view, 1.0);
    ui
}

pub(crate) struct Running {
    ui: TypedUi<Window>,
    scene: Scene,
    view: Size,
    renderer: Option<TestRenderer>,
    pub(crate) session: Rc<Session>,
}

impl Running {
    pub(crate) fn start(command: &str) -> Self {
        Self::with(Options::new(), command)
    }

    pub(crate) fn with(options: Options, command: &str) -> Self {
        let options = options.shell("/bin/sh", [String::from("-c"), String::from(command)]);
        let session =
            Rc::new(Session::open(&options, |_: Report| {}).expect("failed to open PTY session"));
        let mut running = Self {
            ui: open_ui(&session, VIEW),
            scene: Scene::new(),
            view: VIEW,
            renderer: None,
            session,
        };
        running.settle();
        running
    }

    fn pointer(&mut self, at: Point, button: PointerButton, modifiers: Modifiers) {
        self.ui.handle_event(Event::PointerPressed {
            position: at,
            button,
            modifiers,
        });
        self.ui.handle_event(Event::PointerReleased {
            position: at,
            button,
            modifiers,
        });
        self.land_jobs();
    }

    pub(crate) fn click(&mut self, at: Point, modifiers: Modifiers) {
        self.pointer(at, PointerButton::Primary, modifiers);
    }

    pub(crate) fn double_click(&mut self, at: Point) {
        self.click(at, Modifiers::NONE);
        self.click(at, Modifiers::NONE);
    }

    pub(crate) fn right_click(&mut self, at: Point) {
        self.pointer(at, PointerButton::Secondary, Modifiers::NONE);
        self.settle();
    }

    pub(crate) fn settle(&mut self) {
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(Instant::now()));
    }

    pub(crate) fn resize(&mut self, view: Size) {
        self.view = view;
        self.ui.set_viewport(view, 1.0);
        self.settle();
    }

    pub(crate) fn activate(&mut self, active: bool) {
        self.ui.update(|window| window.active = active);
        self.settle();
    }

    pub(crate) fn wheel(&mut self, back: f32) {
        self.ui.handle_event(Event::Scroll {
            position: Point::new(self.view.width / 2.0, self.view.height / 2.0),
            delta: ScrollDelta::from_content(Point::new(0.0, back)),
            modifiers: Modifiers::NONE,
            source: ScrollSource::Wheel,
        });
        self.settle();
    }

    pub(crate) fn chord(&mut self, key: Key, modifiers: Modifiers) {
        self.ui.handle_event(Event::KeyPressed {
            key,
            code: KeyCode::UNKNOWN,
            location: KeyLocation::Standard,
            modifiers,
        });
        self.land_jobs();
    }

    fn land_jobs(&mut self) {
        for job in self.ui.take_jobs() {
            if let Job::Start {
                id,
                work: Work::Blocking(work),
                ..
            } = job
            {
                self.ui.complete_job(JobResult {
                    job: id,
                    output: Some(work()),
                });
            }
        }
    }

    pub(crate) fn swept(&mut self, modifiers: Modifiers) -> String {
        let first = Point::new(0.5, 0.5);
        let last = Point::new(self.view.width - 1.0, self.view.height - 1.0);
        self.drag(first, last, modifiers);
        self.session.selected_text().unwrap_or_default()
    }

    pub(crate) fn drag(&mut self, from: Point, to: Point, modifiers: Modifiers) {
        self.ui.handle_event(Event::PointerPressed {
            position: from,
            button: PointerButton::Primary,
            modifiers,
        });
        self.ui.handle_event(Event::PointerMoved(to));
        self.ui.handle_event(Event::PointerReleased {
            position: to,
            button: PointerButton::Primary,
            modifiers,
        });
    }

    pub(crate) fn wait_for(&mut self, expected: &str, modifiers: Modifiers) -> String {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let output = self.swept(modifiers);
            if output.contains(expected) {
                return output;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {expected:?}, saw {output:?}",
            );
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    pub(crate) fn wait_drawn(&mut self, expected: &[&str]) -> String {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            self.settle();
            let drawn = self.ui.dump();
            if expected.iter().all(|text| drawn.contains(text)) {
                return drawn;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting to draw {expected:?}, drew {drawn}",
            );
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    pub(crate) fn assert_drawn_like_fresh(&mut self, step: &str) {
        self.settle();
        let renderer = self.renderer.get_or_insert_with(|| {
            rikka_test_renderer::renderer("rikka-terminal-test").expect("no Vulkan 1.3 device")
        });
        let (width, height) = (self.view.width as u32, self.view.height as u32);
        let mut target = renderer.create_target(width, height).unwrap();
        renderer.draw(&mut self.scene, &mut target).unwrap();
        let drawn = renderer.read_pixels(&mut target).unwrap();

        let mut fresh = open_ui(&self.session, self.view);
        let mut scene = Scene::new();
        let start = Instant::now();
        fresh.render(Rendering::new(&mut scene).with_time(start));
        fresh.render(Rendering::new(&mut scene).with_time(start + SETTLED));
        let mut target = renderer.create_target(width, height).unwrap();
        renderer.draw(&mut scene, &mut target).unwrap();
        let expected = renderer.read_pixels(&mut target).unwrap();

        let differing = drawn
            .chunks(4)
            .zip(expected.chunks(4))
            .filter(|(drawn, expected)| drawn != expected)
            .count();
        assert_eq!(
            differing, 0,
            "{step}: {differing} pixels differ from a fresh draw"
        );
    }
}
