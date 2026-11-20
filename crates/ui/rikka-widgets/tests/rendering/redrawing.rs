use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::*;
use rikka_widgets::*;

use crate::common::renderer;

const VIEW: u32 = 320;

const FRAME: Duration = Duration::from_millis(16);

const RISE: f32 = 24.0;

#[derive(State)]
struct Feed {
    arrived: bool,
}

fn feed_view(feed: &Feed) -> Element {
    let rows = (0..12).map(|row| text(format!("row {row}")).build());
    let arriving = feed.arrived.then(|| {
        container(text("arrived"))
            .padding(4.0)
            .build()
            .keyed("arrived")
            .transition((fade() + offset(0.0, RISE)).enter(motion::ARRIVAL))
    });
    scrollable(column_of(rows.chain(arriving)).gap(4.0))
        .fill()
        .build()
}

#[test]
fn rising_block_damages_only_itself() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(VIEW, VIEW).unwrap();
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Feed { arrived: false }, feed_view, Theme::dark());
    ui.set_viewport(Size::new(VIEW as f32, VIEW as f32), 1.0);
    let mut now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    now += Duration::from_secs(2);
    ui.render(Rendering::new(&mut scene).with_time(now));
    renderer.draw(&mut scene, &mut target).unwrap();

    ui.update(|feed| feed.arrived = true);
    let mut tallest = Vec::new();
    for _ in 0..12 {
        now += FRAME;
        if ui.needs_render() {
            ui.render(Rendering::new(&mut scene).with_time(now));
        }
        let drawn = renderer.draw(&mut scene, &mut target).unwrap();
        tallest.push(
            drawn
                .damage
                .iter()
                .map(|rect| rect.height())
                .fold(0.0, f32::max),
        );
    }
    let rising = &tallest[3..];
    assert!(
        rising.iter().any(|height| *height > 0.0),
        "rise drew nothing: {tallest:?}"
    );
    assert!(
        rising.iter().all(|height| *height < VIEW as f32 / 2.0),
        "rise damaged most of list: {tallest:?}"
    );
}

#[derive(State)]
struct Working;

fn working_view(_: &Working) -> Element {
    column_of(
        (0..12)
            .map(|row| text(format!("row {row}")).build())
            .chain([spinner().build()]),
    )
    .gap(4.0)
    .padding(8.0)
    .build()
}

#[test]
fn spinner_damages_only_its_bounds() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(VIEW, VIEW).unwrap();
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Working, working_view, Theme::dark());
    ui.set_viewport(Size::new(VIEW as f32, VIEW as f32), 1.0);
    let mut now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    renderer.draw(&mut scene, &mut target).unwrap();

    for frame in 0..8 {
        assert!(ui.needs_render(), "frame {frame} not requested");
        now += FRAME;
        ui.render(Rendering::new(&mut scene).with_time(now));
        let drawn = renderer.draw(&mut scene, &mut target).unwrap();
        let area: f32 = drawn
            .damage
            .iter()
            .map(|rect| rect.width() * rect.height())
            .sum();
        assert!(area > 0.0, "frame {frame} drew nothing");
        assert!(
            area < (VIEW * VIEW) as f32 / 16.0,
            "frame {frame} damaged {area} px² for one spinner"
        );
    }
}

#[derive(State)]
struct Typing {
    document: TextDocument,
}

fn typing_view(typing: &Typing) -> Element {
    column_of(
        (0..12)
            .map(|row| text(format!("row {row}")).build())
            .chain([text_field(&typing.document).autofocus(true).build()]),
    )
    .gap(4.0)
    .padding(8.0)
    .build()
}

const CARET_AREA: f32 = 400.0;

const FADE_OUT: Duration = Duration::from_millis(477);

const FADE_IN: Duration = Duration::from_millis(1007);

const LATER: Duration = Duration::from_secs(20);

struct Blinking {
    renderer: rikka_test_renderer::TestRenderer,
    target: rikka_renderer::OffscreenTarget,
    scene: Scene,
    ui: TypedUi<Typing>,
    start: Instant,
}

impl Blinking {
    fn new() -> Self {
        let mut renderer = renderer();
        let target = renderer.create_target(VIEW, VIEW).unwrap();
        let document = TextDocument::new("hello");
        let mut ui = TypedUi::new(Typing { document }, typing_view, Theme::dark());
        ui.set_viewport(Size::new(VIEW as f32, VIEW as f32), 1.0);
        let mut blinking = Self {
            renderer,
            target,
            scene: Scene::new(),
            ui,
            start: Instant::now(),
        };
        for _ in 0..4 {
            blinking.frame(blinking.start);
        }
        blinking
    }

    fn frame(&mut self, at: Instant) -> f32 {
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(at));
        let drawn = self
            .renderer
            .draw(&mut self.scene, &mut self.target)
            .unwrap();
        drawn
            .damage
            .iter()
            .map(|rect| rect.width() * rect.height())
            .sum()
    }

    fn fade(&mut self, from: Instant) -> Vec<f32> {
        let mut areas = vec![self.frame(from)];
        let mut now = from;
        while self.ui.needs_render() && areas.len() < 16 {
            now += FRAME;
            areas.push(self.frame(now));
        }
        areas
    }

    fn wake(&self) -> Option<Duration> {
        self.ui.next_wake().map(|wake| wake - self.start)
    }
}

#[test]
fn caret_blink_damages_only_caret() {
    let mut blinking = Blinking::new();
    let near = |wake: Option<Duration>, at: Duration| {
        wake.is_some_and(|wake| wake.abs_diff(at) < Duration::from_millis(2))
    };

    assert!(!blinking.ui.needs_render(), "steady caret requested frames");
    assert!(
        near(blinking.wake(), FADE_OUT),
        "caret faded early: {:?}",
        blinking.wake()
    );

    let fading_out = blinking.fade(blinking.start + FADE_OUT);
    assert!(
        fading_out.iter().filter(|area| **area > 0.0).count() >= 4,
        "caret fade too short: {fading_out:?}"
    );
    assert!(
        fading_out.iter().all(|area| *area < CARET_AREA),
        "fading caret damaged outside itself: {fading_out:?}"
    );
    assert!(
        near(blinking.wake(), FADE_IN),
        "hidden caret reappeared early: {:?}",
        blinking.wake()
    );

    blinking.frame(blinking.start + LATER);
    let wake = blinking.wake().expect("caret stopped blinking");
    assert!(
        wake - LATER < Duration::from_millis(1060),
        "caret stopped blinking after {LATER:?}"
    );
    let later = blinking.fade(blinking.start + wake);
    assert!(
        later.iter().filter(|area| **area > 0.0).count() >= 2,
        "caret stopped fading after {LATER:?}: {later:?}"
    );
    assert!(
        later.iter().all(|area| *area < CARET_AREA),
        "fading caret damaged outside itself: {later:?}"
    );
}
