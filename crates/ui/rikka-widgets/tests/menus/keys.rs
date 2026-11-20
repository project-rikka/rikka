use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(480.0, 360.0);
const FRAME: Duration = Duration::from_millis(16);
const FRAMES: usize = 20;
const FRUITS: [&str; 5] = ["Apple", "Banana", "Blueberry", "Cherry", "Damson"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Host {
    Combo,
    MenuBar,
    Context,
}

#[derive(State)]
struct Picking {
    host: Host,
    chosen: Option<usize>,
}

fn fruits() -> Vec<MenuItem> {
    FRUITS
        .iter()
        .enumerate()
        .map(|(row, fruit)| {
            menu_item(*fruit).on_press(move |picking: &mut Picking| picking.chosen = Some(row))
        })
        .collect()
}

fn picking_view(picking: &Picking) -> Element {
    let host = match picking.host {
        Host::Combo => combo(picking.chosen)
            .options(FRUITS)
            .on_select(|picking: &mut Picking, row: usize| picking.chosen = Some(row))
            .width(160.0)
            .any(),
        Host::MenuBar => menu_bar().menu("Fruit", fruits()).any(),
        Host::Context => context_area(text("ground").fill(), fruits).fill().any(),
    };
    column! {
        host;
        button("Next").on_press(|_: &mut Picking| {});
    }
    .padding(10.0)
    .fill()
    .build()
}

struct Picker {
    ui: TypedUi<Picking>,
    scene: Scene,
    now: Instant,
}

impl Picker {
    fn opened(host: Host) -> Picker {
        let mut ui = TypedUi::new(Picking { host, chosen: None }, picking_view, Theme::dark());
        ui.set_viewport(VIEW, 1.0);
        let mut picker = Picker {
            ui,
            scene: Scene::new(),
            now: Instant::now(),
        };
        picker.settle();
        let button = match host {
            Host::Context => PointerButton::Secondary,
            Host::Combo | Host::MenuBar => PointerButton::Primary,
        };
        picker.click(Point::new(24.0, 24.0), button);
        picker
    }

    fn settle(&mut self) {
        for _ in 0..FRAMES {
            self.now += FRAME;
            self.ui
                .render(Rendering::new(&mut self.scene).with_time(self.now));
        }
    }

    fn click(&mut self, position: Point, button: PointerButton) {
        let modifiers = Modifiers::NONE;
        for event in [
            Event::PointerMoved(position),
            Event::PointerPressed {
                position,
                button,
                modifiers,
            },
            Event::PointerReleased {
                position,
                button,
                modifiers,
            },
        ] {
            self.ui.handle_event(event);
        }
        self.settle();
    }

    fn press(&mut self, key: Key) {
        self.press_with(key, Modifiers::NONE);
    }

    fn press_with(&mut self, key: Key, modifiers: Modifiers) {
        self.ui.handle_event(Event::KeyPressed {
            key,
            code: KeyCode::UNKNOWN,
            location: KeyLocation::Standard,
            modifiers,
        });
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.now));
    }
}

#[test]
fn every_menu_host_walks_and_types_ahead_alike() {
    let cases: [(&[Key], usize); 6] = [
        (&[Key::ArrowDown], 0),
        (&[Key::End], 4),
        (&[Key::End, Key::Home], 0),
        (&[Key::Char('b')], 1),
        (&[Key::Char('b'), Key::Char('b')], 2),
        (&[Key::Char('B'), Key::Char('l')], 2),
    ];
    for host in [Host::Combo, Host::MenuBar, Host::Context] {
        for (keys, expected) in cases {
            let mut picker = Picker::opened(host);
            for key in keys {
                picker.press(*key);
            }
            picker.press(Key::Enter);
            assert_eq!(
                picker.ui.state().chosen,
                Some(expected),
                "{host:?} after {keys:?}"
            );
        }
    }
}

#[test]
fn escape_closes_every_menu_host_without_choosing() {
    for host in [Host::Combo, Host::MenuBar, Host::Context] {
        let mut picker = Picker::opened(host);
        picker.press(Key::ArrowDown);
        picker.press(Key::Escape);
        picker.settle();
        picker.press(Key::Enter);
        assert_eq!(picker.ui.state().chosen, None, "{host:?}");
    }
}

#[test]
fn focus_leaving_closes_every_menu_host_without_choosing() {
    for host in [Host::Combo, Host::MenuBar, Host::Context] {
        let mut picker = Picker::opened(host);
        picker.press(Key::ArrowDown);
        picker.press(Key::Tab);
        picker.settle();
        picker.press_with(Key::Tab, Modifiers::SHIFT);
        picker.press(Key::Enter);
        assert_eq!(picker.ui.state().chosen, None, "{host:?}");
    }
}
