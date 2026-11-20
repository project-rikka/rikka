use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Color, Point, Rect, Size};
use rikka_ui::{
    Element, Layout, Length, PaintCtx, Quad, Rendering, State, TypedUi, View, Watchers, Widget,
};

const VIEW: Size = Size::new(400.0, 300.0);

const NATURAL: Size = Size::new(200.0, 100.0);

struct Leaf;

impl Widget for Leaf {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(NATURAL.width), Length::px(NATURAL.height))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(bounds).fill(Color::WHITE));
        ctx.draw_pane(Quad::new(bounds).radius(8.0));
    }

    fn same(&self, _other: &Self) -> bool {
        true
    }
}

struct Framer {
    into: Rect,
}

#[derive(Default)]
struct Framed {
    shift: Point,
}

impl Widget for Framer {
    type State = Framed;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, state: &mut Framed, ctx: &mut PaintCtx<'_>) {
        ctx.set_subtree_scale(self.into.width() / NATURAL.width, self.into.origin);
        state.shift = self.into.origin - ctx.bounds().origin;
    }

    fn children_offset(&self, state: &Framed, _bounds: Rect, _content: Size) -> Point {
        state.shift
    }
}

#[derive(Default, State)]
struct Framing {
    into: Rect,
}

fn framing(framing: &Framing) -> impl View {
    Element::with_children(Framer { into: framing.into }, vec![Element::new(Leaf)])
}

fn leaf_quad(ui: &TypedUi<Framing>) -> String {
    let dump = ui.dump();
    let mut lines = dump.lines().skip_while(|line| !line.contains("Leaf"));
    lines
        .find(|line| line.contains("· quad"))
        .map(|line| line.trim().to_owned())
        .unwrap_or_default()
}

#[test]
fn scaled_subtree_draws_inside_target() {
    let into = |x: f32, y: f32, width: f32| Rect::new(x, y, width, width / 2.0);
    let mut ui = TypedUi::new(
        Framing {
            into: into(40.0, 30.0, 100.0),
        },
        framing,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));

    for (step, target) in [
        into(40.0, 30.0, 100.0),
        into(120.0, 90.0, 260.0),
        into(0.0, 0.0, 200.0),
    ]
    .into_iter()
    .enumerate()
    {
        ui.update(|framing| framing.into = target);
        let at = now + Duration::from_millis(16 * (step as u64 + 1));
        ui.render(Rendering::new(&mut scene).with_time(at));
        let expected = format!(
            "· quad [{:.2} {:.2} {:.2}x{:.2}]",
            target.x(),
            target.y(),
            target.width(),
            target.height()
        );
        assert_eq!(leaf_quad(&ui), expected, "leaf not anchored to {target:?}");
        let near = |pane: Rect| {
            [
                (pane.x(), target.x()),
                (pane.y(), target.y()),
                (pane.width(), target.width()),
                (pane.height(), target.height()),
            ]
            .into_iter()
            .all(|(drawn, target)| (drawn - target).abs() < 0.01)
        };
        assert!(
            ui.glass().iter().any(|pane| near(pane.rect)),
            "glass pane not scaled into {target:?}: {:?}",
            ui.glass()
        );
    }
}

struct Leader {
    stage: u32,
    shared: Rc<Cell<u32>>,
    resting: Watchers,
}

impl Widget for Leader {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, _state: &mut (), _ctx: &mut PaintCtx<'_>) {
        if self.shared.replace(self.stage) != self.stage {
            self.resting.repaint();
        }
    }
}

struct Follower {
    shared: Rc<Cell<u32>>,
    seen: Rc<Cell<u32>>,
    resting: Watchers,
}

impl Widget for Follower {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::px(10.0), Length::px(10.0))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        self.seen.set(self.shared.get());
        self.resting.watch(ctx.handle());
    }

    fn same(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Default, State)]
struct Following {
    stage: u32,
    shared: Rc<Cell<u32>>,
    seen: Rc<Cell<u32>>,
    resting: Watchers,
}

fn following(following: &Following) -> impl View {
    let follower = Follower {
        shared: Rc::clone(&following.shared),
        seen: Rc::clone(&following.seen),
        resting: following.resting.clone(),
    };
    Element::with_children(
        Leader {
            stage: following.stage,
            shared: Rc::clone(&following.shared),
            resting: following.resting.clone(),
        },
        vec![Element::new(follower)],
    )
}

#[test]
fn child_woken_by_parent_paints_same_render() {
    let mut ui = TypedUi::new(Following::default(), following, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));

    for stage in 1..4 {
        ui.update(|following| following.stage = stage);
        let at = now + Duration::from_millis(16 * u64::from(stage));
        ui.render(Rendering::new(&mut scene).with_time(at));
        assert_eq!(
            ui.state().seen.get(),
            stage,
            "follower saw stale parent layout"
        );
    }
}
