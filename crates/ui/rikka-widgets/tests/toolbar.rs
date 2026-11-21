use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::*;

const HEIGHT: f32 = 300.0;
const ROW: f32 = 19.0;
const SLOT: f32 = 38.0;
const FIRST: f32 = 23.0;

#[derive(State, Default)]
struct Bench {
    pressed: Vec<&'static str>,
    volume: f32,
}

fn tool(name: &'static str) -> ToolButton {
    tool_button(MaterialSymbol::STAR, name)
        .on_press(move |bench: &mut Bench| bench.pressed.push(name))
}

fn bench(bench: &Bench) -> impl View {
    column_of([toolbar()
        .group(tool_group([tool("pinned")]).priority(ToolPriority::Pinned))
        .group(tool_group([tool("high")]).priority(ToolPriority::High))
        .group(tool_group([tool("low")]).priority(ToolPriority::Low))
        .group(
            tool_group([tool_item(spacer().width(30.0)).overflow([menu_slider(
                "volume",
                bench.volume,
                0.0..=1.0,
            )
            .step(0.25)
            .on_change(|bench: &mut Bench, volume: f32| bench.volume = volume)])])
            .priority(ToolPriority::Normal),
        )])
    .fill()
}

fn shown(width: f32) -> (TypedUi<Bench>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Bench::default(), bench, Theme::dark());
    ui.set_viewport(Size::new(width, HEIGHT), 1.0);
    ui.render(&mut scene);
    ui.render(&mut scene);
    (ui, scene)
}

fn click<S: State>(ui: &mut TypedUi<S>, scene: &mut Scene, slot: usize) {
    let at = Point::new(FIRST + SLOT * slot as f32, ROW);
    ui.handle_event(Event::PointerMoved(at));
    ui.handle_event(Event::PointerPressed {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.handle_event(Event::PointerReleased {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(scene);
}

fn press<S: State>(ui: &mut TypedUi<S>, scene: &mut Scene, key: Key) {
    ui.handle_event(Event::KeyPressed {
        key,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers: Modifiers::NONE,
    });
    ui.render(scene);
}

#[test]
fn groups_overflow_from_the_lowest_priority() {
    let cases: [(f32, usize, &[Key], &[&str]); 4] = [
        (200.0, 2, &[], &["low"]),
        (150.0, 1, &[], &["high"]),
        (150.0, 2, &[Key::ArrowDown, Key::Enter], &["low"]),
        (60.0, 0, &[], &["pinned"]),
    ];
    for (width, slot, keys, expected) in cases {
        let (mut ui, mut scene) = shown(width);
        click(&mut ui, &mut scene, slot);
        for key in keys {
            press(&mut ui, &mut scene, *key);
        }
        assert_eq!(ui.state().pressed, expected, "{width} px, slot {slot}");
    }
}

#[test]
fn overflowed_sliders_follow_arrow_keys() {
    let (mut ui, mut scene) = shown(150.0);
    click(&mut ui, &mut scene, 2);
    for key in [Key::ArrowUp, Key::ArrowRight, Key::ArrowRight] {
        press(&mut ui, &mut scene, key);
    }
    assert_eq!(ui.state().volume, 0.5);
}

#[derive(State, Default)]
struct Tabs {
    pinned: usize,
    pressed: Vec<&'static str>,
}

fn tabs(tabs: &Tabs) -> impl View {
    let mut bar = toolbar();
    for (index, name) in ["a", "b", "c", "d"].into_iter().enumerate() {
        let tab = tool_button(MaterialSymbol::STAR, name).on_press(move |tabs: &mut Tabs| {
            tabs.pressed.push(name);
            tabs.pinned = index;
        });
        let priority = match index == tabs.pinned {
            true => ToolPriority::Pinned,
            false => ToolPriority::Normal,
        };
        bar = bar.group(tool_group([tab]).priority(priority));
    }
    column_of([bar]).fill()
}

#[test]
fn newly_pinned_group_comes_out_of_overflow() {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Tabs::default(), tabs, Theme::dark());
    ui.set_viewport(Size::new(150.0, HEIGHT), 1.0);
    ui.render(&mut scene);
    ui.render(&mut scene);
    click(&mut ui, &mut scene, 2);
    for key in [Key::ArrowDown, Key::ArrowDown, Key::Enter] {
        press(&mut ui, &mut scene, key);
    }
    ui.render(&mut scene);
    click(&mut ui, &mut scene, 1);
    assert_eq!(ui.state().pressed, ["d", "d"]);
}
