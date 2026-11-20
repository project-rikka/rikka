use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(300.0, 300.0);
const ROW: f32 = 20.0;
const BODY: f32 = 60.0;
const TALL: f32 = 400.0;
const FRAME: Duration = Duration::from_millis(16);
const FRAMES: usize = 60;
const LEADING: usize = (VIEW.height / ROW) as usize - 2;
const TRAILING: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Filler,
    Header,
    Body,
    Footer,
}

#[derive(Clone, Copy, Debug)]
struct Painted {
    part: Part,
    at: f32,
    from: f32,
    to: f32,
}

#[derive(Default)]
struct Seen {
    painted: RefCell<Vec<Painted>>,
}

struct Marker {
    part: Part,
    height: f32,
    seen: Rc<Seen>,
}

impl Widget for Marker {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(self.height))
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let visible = ctx.visible();
        self.seen.painted.borrow_mut().push(Painted {
            part: self.part,
            at: ctx.bounds().y(),
            from: visible.y(),
            to: visible.y() + visible.height(),
        });
    }
}

#[derive(Clone, Copy, Debug)]
enum Holder {
    Column,
    List { leading: usize },
}

#[derive(State)]
struct Fold {
    parts: Parts,
    holder: Holder,
}

#[derive(Clone)]
struct Parts {
    open: bool,
    body: f32,
    reveals: bool,
    seen: Rc<Seen>,
}

impl Parts {
    fn marker(&self, part: Part, height: f32) -> Element {
        Element::new(Marker {
            part,
            height,
            seen: Rc::clone(&self.seen),
        })
    }

    fn collapsible(&self) -> Element {
        let body = self.open.then(|| self.marker(Part::Body, self.body));
        let collapsible = collapsible(body);
        match self.reveals {
            true => collapsible.reveals_on_open(),
            false => collapsible,
        }
        .fill_width()
        .any()
    }
}

fn fold_view(fold: &Fold) -> Element {
    let parts = fold.parts.clone();
    match fold.holder {
        Holder::Column => column! {
            parts.marker(Part::Header, ROW);
            parts.collapsible();
            parts.marker(Part::Footer, ROW);
        }
        .fill()
        .build(),
        Holder::List { leading } => {
            lazy_column(leading + 2 + TRAILING, move |slot| match slot.index {
                at if at == leading => column! {
                    parts.marker(Part::Header, ROW);
                    parts.collapsible();
                }
                .fill_width()
                .any(),
                at if at == leading + 1 => parts.marker(Part::Footer, ROW),
                _ => parts.marker(Part::Filler, ROW),
            })
            .fill()
            .build()
        }
    }
}

struct Watch {
    ui: TypedUi<Fold>,
    scene: Scene,
    now: Instant,
    seen: Rc<Seen>,
}

impl Watch {
    fn new(holder: Holder, open: bool) -> Watch {
        Watch::with(holder, open, BODY, false)
    }

    fn with(holder: Holder, open: bool, body: f32, reveals: bool) -> Watch {
        let seen = Rc::new(Seen::default());
        let mut ui = TypedUi::new(
            Fold {
                parts: Parts {
                    open,
                    body,
                    reveals,
                    seen: Rc::clone(&seen),
                },
                holder,
            },
            fold_view,
            Theme::dark(),
        );
        ui.set_viewport(VIEW, 1.0);
        let mut watch = Watch {
            ui,
            scene: Scene::new(),
            now: Instant::now(),
            seen,
        };
        watch.rests();
        watch
    }

    fn frame(&mut self, by: Duration) -> Vec<Painted> {
        self.seen.painted.borrow_mut().clear();
        self.now += by;
        self.ui.request_frame();
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.now));
        self.seen
            .painted
            .borrow()
            .iter()
            .filter(|painted| painted.to - painted.from > 0.5)
            .copied()
            .collect()
    }

    fn rests(&mut self) -> Vec<Painted> {
        (0..8)
            .map(|_| self.frame(Duration::from_millis(200)))
            .last()
            .unwrap_or_default()
    }
}

fn latest_part(shown: &[Painted], part: Part) -> Option<Painted> {
    shown
        .iter()
        .rev()
        .find(|painted| painted.part == part)
        .copied()
}

fn bodies(shown: &[Painted]) -> impl Iterator<Item = (Painted, Painted)> + '_ {
    shown.iter().enumerate().filter_map(|(at, body)| {
        let footer = shown[at + 1..]
            .iter()
            .find(|painted| painted.part == Part::Footer)?;
        (body.part == Part::Body).then_some((*body, *footer))
    })
}

fn assert_rests(settled: &[Painted], open: bool, case: impl std::fmt::Debug) {
    let footer = latest_part(settled, Part::Footer).expect("footer missing");
    let expected = match open {
        true => ROW + BODY,
        false => ROW,
    };
    assert!(
        (footer.at - expected).abs() < 0.5,
        "{case:?} open {open}: {footer:?}"
    );
    assert_eq!(
        latest_part(settled, Part::Body).is_some(),
        open,
        "{case:?} open {open}"
    );
}

#[test]
fn folding_slides_under_top_edge() {
    for open in [false, true] {
        let mut watch = Watch::new(Holder::Column, !open);
        watch.ui.update(|fold| fold.parts.open = open);
        let frames: Vec<Vec<Painted>> = (0..FRAMES).map(|_| watch.frame(FRAME)).collect();
        let settled = watch.rests();

        let drawn: Vec<(Painted, Painted)> =
            frames.iter().flat_map(|shown| bodies(shown)).collect();
        let sliding = drawn
            .iter()
            .filter(|(body, _)| body.to - body.from < BODY - 0.5)
            .count();
        assert!(
            sliding > 2,
            "open {open}: body not partially drawn mid-animation"
        );
        for (body, footer) in drawn {
            assert!(
                body.from >= ROW - 0.5 && body.to <= footer.at + 0.5,
                "open {open}: body outside header and footer: {body:?}"
            );
            assert!(
                (body.at + BODY - body.to).abs() < 0.5,
                "open {open}: body upper edge shown: {body:?}"
            );
        }

        assert_rests(&settled, open, Holder::Column);
    }
}

#[test]
fn folded_content_reopens() {
    for holder in [Holder::Column, Holder::List { leading: 0 }] {
        let mut watch = Watch::new(holder, false);
        for open in [true, false, true, false, true] {
            watch.ui.update(|fold| fold.parts.open = open);
            assert_rests(&watch.rests(), open, holder);
        }
    }
}

#[test]
fn collapsible_near_bottom_scrolls_into_view() {
    let cases = [(BODY, VIEW.height - ROW - BODY), (TALL, 0.0)];
    for (body, header) in cases {
        let mut watch = Watch::with(Holder::List { leading: LEADING }, false, body, true);
        watch.ui.update(|fold| fold.parts.open = true);
        let frames: Vec<Vec<Painted>> = (0..FRAMES).map(|_| watch.frame(FRAME)).collect();
        let settled = watch.rests();

        let shown = latest_part(&settled, Part::Header).expect("header not in view");
        assert!(
            (shown.at - header).abs() < 0.5,
            "body {body}: header not at {header}: {shown:?}"
        );
        let body_span = latest_part(&settled, Part::Body).expect("body not in view");
        assert!(
            body_span.to - body_span.from >= body.min(VIEW.height - ROW) - 0.5,
            "body {body}: visible body clipped wrongly: {body_span:?}"
        );
        for shown in frames
            .iter()
            .filter_map(|shown| latest_part(shown, Part::Header))
        {
            assert!(
                shown.at >= -0.5,
                "body {body}: header moved off top: {shown:?}"
            );
        }
    }
}
