use rikka_effects::{Effect, Kind};
use rikka_effects_forms::Tuning;
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_ui::{Element, Event, Modifiers, Point, PointerButton, Size, State, TypedUi};

const VIEW: Size = Size::new(320.0, 1600.0);

#[derive(State)]
struct Host {
    effect: Effect,
    previews: Vec<Effect>,
    commits: Vec<Effect>,
}

impl Host {
    fn new(kind: Kind) -> Host {
        Host {
            effect: kind.default_effect(),
            previews: Vec::new(),
            commits: Vec::new(),
        }
    }
}

fn host_view(host: &Host) -> Element {
    let tuning = Tuning::new(
        |host: &mut Host, effect: Effect| host.previews.push(effect),
        |host: &mut Host, effect: Effect| {
            host.effect = effect.clone();
            host.commits.push(effect);
        },
    );
    rikka_effects_forms::form(&host.effect, &tuning)
}

fn open_form(kind: Kind) -> (TypedUi<Host>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Host::new(kind), host_view, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    (ui, scene)
}

fn clicked(ui: &mut TypedUi<Host>, scene: &mut Scene, at: Point) {
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

#[test]
fn slider_previews_and_commits_on_release() {
    let (mut ui, mut scene) = open_form(Kind::GaussianBlur);

    let mut y = 0.0;
    while ui.state().commits.is_empty() && y < 120.0 {
        clicked(&mut ui, &mut scene, Point::new(150.0, y));
        y += 2.0;
    }

    let host = ui.state();
    let committed = host.commits.last().expect("release commits");
    let radius = committed.value("radius").expect("radius");
    assert!(radius > 1.0, "{radius}");
    assert_eq!(host.previews.last(), Some(committed));
}

fn dragged(ui: &mut TypedUi<Host>, scene: &mut Scene, from: Point) {
    ui.handle_event(Event::PointerMoved(from));
    ui.handle_event(Event::PointerPressed {
        position: from,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(&mut *scene);
    let mut at = from;
    for _ in 0..4 {
        at = Point::new(at.x + 6.0, at.y + 2.0);
        ui.handle_event(Event::PointerMoved(at));
        ui.render(&mut *scene);
    }
    ui.handle_event(Event::PointerReleased {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(&mut *scene);
}

#[test]
fn pad_drag_commits_once() {
    let (mut ui, mut scene) = open_form(Kind::RadialBlur);

    let mut y = 600.0;
    while ui.state().commits.is_empty() && y > 0.0 {
        dragged(&mut ui, &mut scene, Point::new(30.0, y));
        y -= 4.0;
    }

    let host = ui.state();
    assert_eq!(host.commits.len(), 1, "one drag is one undo step");
    let centre = host.commits[0].value("center_x").expect("center_x");
    assert!(centre < 0.5, "{centre}");
    assert!(host.previews.len() >= 4, "{}", host.previews.len());
}
