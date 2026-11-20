use std::time::{Duration, Instant};

use rikka_renderer::{OffscreenTarget, Preparer, Renderer, Scene, SceneFrame};
use rikka_test_renderer::TestRenderer;
use rikka_theme::{MaterialYou, Theme, Variant};
use rikka_types::{Color, Modifiers, Point, ScrollDelta, Size};
use rikka_ui::*;
use rikka_widgets::*;

use crate::common::{UNCOVERED, pixel, renderer};

const VIEW: u32 = 320;

const FRAME: Duration = Duration::from_millis(16);

#[derive(Clone, State)]
struct Page {
    label: String,
    lead: f32,
    rows: Vec<String>,
    looking: usize,
    ahead: usize,
    pushable: bool,
}

fn page_view(page: &Page) -> Element {
    let label = text(page.label.clone())
        .build()
        .keyed(page.label.clone())
        .transition(fade());
    let looking = segmented(page.looking)
        .segments(["Changes", "History"])
        .glyphs([MaterialSymbol::EDIT_DOCUMENT, MaterialSymbol::HISTORY])
        .control_size(ControlSize::Small)
        .fill_width()
        .build();
    let mut pushed = vec![
        icon(MaterialSymbol::CLOUD_UPLOAD)
            .extent(Length::px(16.0))
            .build(),
        text("Push").caption().no_wrap().build(),
    ];
    if page.ahead > 0 {
        pushed.push(text(page.ahead.to_string()).caption().no_wrap().build());
    }
    let push = button(row_of(pushed).gap(4.0).align_items(Align::Center))
        .secondary()
        .small()
        .enabled(page.pushable)
        .fill_width()
        .build();
    let rows = page.rows.iter().map(|row| text(row.clone()).build());
    column_of(
        [
            looking,
            container(label)
                .glass()
                .padding(Inset::exact(page.lead * 0.5, 8.0, 8.0, page.lead))
                .build(),
        ]
        .into_iter()
        .chain(rows)
        .chain([push]),
    )
    .gap(4.0)
    .build()
}

struct Dice(u64);

impl Dice {
    fn roll(&mut self, sides: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % sides as u64) as usize
    }
}

fn change(page: &mut Page, dice: &mut Dice) {
    let row = dice.roll(page.rows.len().max(1));
    match dice.roll(8) {
        0 => {
            page.label = ["Push 9", "↑ Push · 95", "Push", "最適化してから"][dice.roll(4)].into();
        }
        1 => page.lead = 2.0 + dice.roll(40) as f32 * 0.37,
        2 => page.looking = 1 - page.looking,
        3 => page.ahead = [0, 3, 9, 95][dice.roll(4)],
        4 => page.pushable = !page.pushable,
        5 if page.rows.len() < 9 => {
            page.rows
                .insert(row, format!("row {} を足した", dice.roll(100)));
        }
        6 if page.rows.len() > 1 => {
            page.rows.remove(row);
        }
        _ => page.rows[row].push_str([" and more", "、", "?"][dice.roll(3)]),
    }
}

const STEPS: usize = 48;

fn opening() -> Page {
    Page {
        label: "Push 9".into(),
        lead: 8.0,
        rows: (0..6).map(|row| format!("row {row} 最適化")).collect(),
        looking: 0,
        ahead: 3,
        pushable: true,
    }
}

fn open_ui(page: &Page, theme: &Theme) -> TypedUi<Page> {
    let mut ui = TypedUi::new(page.clone(), page_view, theme.clone());
    ui.set_background(Some(UNCOVERED));
    ui.set_viewport(Size::new(VIEW as f32, VIEW as f32), 1.0);
    ui
}

fn fresh(renderer: &mut Renderer, page: &Page, theme: &Theme) -> Vec<u8> {
    let mut target = renderer.create_target(VIEW, VIEW).unwrap();
    let mut scene = Scene::new();
    let mut ui = open_ui(page, theme);
    let start = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(start));
    ui.render(Rendering::new(&mut scene).with_time(start + Duration::from_secs(2)));
    renderer.draw(&mut scene, &mut target).unwrap();
    renderer.read_pixels(&mut target).unwrap()
}

fn differing(drawn: &[u8], expected: &[u8]) -> Option<(usize, (usize, usize, usize, usize))> {
    let mut count = 0;
    let mut bounds = (usize::MAX, usize::MAX, 0, 0);
    for (index, (a, b)) in drawn.chunks(4).zip(expected.chunks(4)).enumerate() {
        if a.iter().zip(b).any(|(a, b)| a.abs_diff(*b) > 24) {
            count += 1;
            let (x, y) = (index % VIEW as usize, index / VIEW as usize);
            bounds = (
                bounds.0.min(x),
                bounds.1.min(y),
                bounds.2.max(x),
                bounds.3.max(y),
            );
        }
    }
    (count > 0).then_some((count, bounds))
}

struct Replay {
    renderer: TestRenderer,
    target: OffscreenTarget,
    scene: Scene,
    mirror: Scene,
    frame: SceneFrame,
    preparer: Preparer,
    ui: TypedUi<Page>,
    now: Instant,
}

impl Replay {
    fn open(theme: &Theme) -> Replay {
        let mut renderer = renderer();
        let target = renderer.create_target(VIEW, VIEW).unwrap();
        let preparer = renderer.preparer();
        Replay {
            renderer,
            target,
            scene: Scene::new(),
            mirror: Scene::new(),
            frame: SceneFrame::default(),
            preparer,
            ui: open_ui(&opening(), theme),
            now: Instant::now(),
        }
    }

    fn settle(&mut self) -> Vec<u8> {
        for _ in 0..240 {
            self.now += FRAME;
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

fn assert_replay_matches_fresh(theme: &Theme, pages: &[Page]) {
    let mut replay = Replay::open(theme);
    for (at, page) in pages.iter().enumerate() {
        let changed = page.clone();
        replay.ui.update(|state| *state = changed);
        let drawn = replay.settle();
        let expected = fresh(&mut replay.renderer, page, theme);
        if let Some((count, bounds)) = differing(&drawn, &expected) {
            panic!("change {at}: {count} pixels in {bounds:?} differ from fresh draw");
        }
    }
}

fn wandering(seed: u64) -> Vec<Page> {
    let mut dice = Dice(seed);
    let mut page = opening();
    (0..STEPS)
        .map(|_| {
            change(&mut page, &mut dice);
            page.clone()
        })
        .collect()
}

fn liquid() -> Theme {
    let liquid = MaterialYou::new(Color::rgb(0.78, 0.56, 0.82))
        .spec(Variant::Light)
        .liquid();
    Theme::from_spec(&liquid)
}

#[test]
fn incremental_frames_match_fresh_draw() {
    for (theme, seed) in [(Theme::dark(), 7), (liquid(), 24), (Theme::dark(), 39)] {
        assert_replay_matches_fresh(&theme, &wandering(seed));
    }
}

#[test]
fn removed_node_leaves_no_pixels() {
    let mut page = opening();
    let mut pages = Vec::new();
    page.ahead = 0;
    pages.push(page.clone());
    page.rows.remove(2);
    pages.push(page.clone());
    page.ahead = 9;
    pages.push(page);
    assert_replay_matches_fresh(&Theme::dark(), &pages);
}

const CARDS: usize = 6;

const CARD: f32 = 80.0;

const CARD_INSET: f32 = 10.0;

const CARD_RADIUS: f32 = 14.0;

const STROKE: f32 = 10.0;

#[derive(State)]
struct Cards;

fn cards_view(_: &Cards) -> Element {
    let cards: Vec<Element> = (0..CARDS)
        .map(|_| {
            let block = container(spacer())
                .height(CARD - 2.0 * CARD_INSET)
                .fill_width()
                .background(Color::WHITE);
            container(container(block).clip_content().fill_width())
                .padding(CARD_INSET)
                .radius(CARD_RADIUS)
                .clip_content()
                .height(CARD)
                .fill_width()
                .background(Color::BLACK)
                .build()
        })
        .collect();
    scrollable(column_of(cards).fill_width()).fill().build()
}

#[test]
fn rounded_clip_content_follows_whole_pixel_scroll() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(VIEW, VIEW).unwrap();
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Cards, cards_view, Theme::dark());
    let view = VIEW as f32;
    ui.set_viewport(Size::new(view, view), 1.0);
    let mut now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    renderer.draw(&mut scene, &mut target).unwrap();

    let travel = CARDS as f32 * CARD - view;
    let strokes = (travel / STROKE) as usize;
    let phases = std::iter::once(ScrollPhase::Began)
        .chain(std::iter::repeat_n(ScrollPhase::Moved, strokes))
        .chain(std::iter::once(ScrollPhase::Ended));
    for phase in phases {
        let down = match phase {
            ScrollPhase::Moved => STROKE,
            ScrollPhase::Began | ScrollPhase::Ended => 0.0,
        };
        ui.handle_event(Event::Scroll {
            source: ScrollSource::Continuous(phase),
            position: Point::new(view / 2.0, view / 2.0),
            delta: ScrollDelta::from_offset(Point::new(0.0, down)),
            modifiers: Modifiers::NONE,
        });
        now += FRAME;
        ui.render(Rendering::new(&mut scene).with_time(now));
        renderer.draw(&mut scene, &mut target).unwrap();
    }

    let pixels = renderer.read_pixels(&mut target).unwrap();
    let last = view - CARD / 2.0;
    let (red, green, blue, _) = pixel(&pixels, VIEW as usize, VIEW as usize / 2, last as usize);
    assert!(
        red > 200 && green > 200 && blue > 200,
        "last card content missing after scrolling: ({red}, {green}, {blue})"
    );
}
