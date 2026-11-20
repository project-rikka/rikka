use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Point, Rect, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(480.0, 200.0);

const SETTLED: Duration = Duration::from_secs(2);

const GROUP: &str = "rikka.test.tab";

struct Carried;

const GUTTER: f32 = 6.0;

const RIM: f32 = 1.0;

const STRIP: f32 = 30.0;

type Watch = Rc<Cell<Rect>>;

struct Body {
    at: Watch,
}

impl Widget for Body {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().grow(1.0)
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        self.at.set(ctx.bounds());
    }
}

#[derive(State)]
struct Window {
    divided: bool,
    ratio: f32,
    landings: Vec<(u8, DropTarget)>,
    shown: [Watch; 2],
}

impl Window {
    fn whole() -> Self {
        Self {
            divided: false,
            ratio: 0.5,
            landings: Vec::new(),
            shown: [Watch::default(), Watch::default()],
        }
    }

    fn divided() -> Self {
        Self {
            divided: true,
            ..Self::whole()
        }
    }

    fn pane_of(&self, which: u8) -> Element {
        let body = Element::new(Body {
            at: self.shown[usize::from(which)].clone(),
        });
        pane(
            container(
                column! {
                    spacer().height(Length::px(STRIP)).shrink(0.0);
                    body
                }
                .fill(),
            )
            .padding(Inset::all(RIM))
            .grow(1.0),
        )
        .accepting(
            GROUP,
            move |window: &mut Window, landing: DropTarget, Carried| {
                window.landings.push((which, landing))
            },
        )
        .build()
    }
}

fn windowed(window: &Window) -> impl View {
    if !window.divided {
        return window.pane_of(0);
    }
    split(
        Axis::Horizontal,
        window.ratio,
        window.pane_of(0),
        window.pane_of(1),
    )
    .on_resize(|window: &mut Window, ratio: f32| window.ratio = ratio)
    .build()
}

fn window(state: Window) -> (TypedUi<Window>, Scene) {
    let mut ui = TypedUi::new(state, windowed, Theme::dark());
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

fn mime() -> String {
    format!(
        "application/x-rikka-tab;group={GROUP};pid={}",
        std::process::id()
    )
}

fn hover(ui: &mut TypedUi<Window>, scene: &mut Scene, at: Point, mime: &str) -> Option<String> {
    ui.handle_event(Event::DragMoved {
        position: at,
        offer: DragOffer::new([mime.to_owned()]),
    });
    settle(ui, scene);
    ui.drag_accepts().map(|accept| accept.mime.clone())
}

fn drop_at(ui: &mut TypedUi<Window>, scene: &mut Scene, at: Point) {
    let mime = mime();
    hover(ui, scene, at, &mime);
    set_drag_payload(Carried);
    ui.handle_event(Event::DragDropped {
        position: at,
        offer: DragOffer::new([mime]),
        action: DragAction::Move,
    });
    settle(ui, scene);
}

#[test]
fn tab_dropped_on_pane_edge_splits_pane() {
    let (mut ui, mut scene) = window(Window::whole());

    drop_at(&mut ui, &mut scene, Point::new(6.0, 100.0));
    drop_at(&mut ui, &mut scene, Point::new(474.0, 100.0));
    drop_at(&mut ui, &mut scene, Point::new(240.0, 6.0));
    drop_at(&mut ui, &mut scene, Point::new(240.0, 194.0));

    assert_eq!(
        ui.state().landings,
        [
            (0, DropTarget::Split(Side::Left)),
            (0, DropTarget::Split(Side::Right)),
            (0, DropTarget::Split(Side::Top)),
            (0, DropTarget::Split(Side::Bottom)),
        ]
    );
}

#[test]
fn split_distributes_every_pixel() {
    let (ui, _scene) = window(Window::divided());
    let half = (VIEW.width - GUTTER) / 2.0;

    let left = ui.state().shown[0].get();
    assert_eq!(left.x(), RIM);
    assert_eq!(left.width(), half - RIM * 2.0);
    assert_eq!(left.y(), RIM + STRIP, "strip lost its height");
    assert_eq!(left.height(), VIEW.height - RIM * 2.0 - STRIP);

    let right = ui.state().shown[1].get();
    assert_eq!(right.x(), half + GUTTER + RIM);
    assert_eq!(right.width(), left.width());
    assert_eq!(right.height(), left.height());
}
