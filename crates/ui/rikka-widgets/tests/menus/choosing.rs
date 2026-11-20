use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(400.0, 300.0);
const TRIGGER: &str = "trigger";
const FRUITS: [&str; 5] = ["Apple", "Banana", "Blueberry", "Cherry", "Damson"];

#[derive(State)]
struct Picker {
    highlighted: Option<usize>,
    chosen: Option<(usize, Activation)>,
}

fn picking(picker: &Picker) -> impl View {
    let rows = FRUITS
        .iter()
        .enumerate()
        .map(|(row, fruit)| menu_item(*fruit).enabled(row != 4));
    column! {
        spacer().width(120.0).height(20.0).position(10.0, 10.0).build().keyed(TRIGGER);
        if picker.chosen.is_none() {
            menu()
                .items(rows)
                .highlighted(picker.highlighted)
                .on_choose(|picker: &mut Picker, row: usize, activation: Activation| {
                    picker.chosen = Some((row, activation));
                })
                .on_dismiss(|_: &mut Picker| {})
                .build()
                .anchored(Anchor::below(TRIGGER));
        }
    }
    .fill()
}

fn opened(highlighted: Option<usize>) -> (TypedUi<Picker>, Scene) {
    let mut scene = Scene::new();
    let picker = Picker {
        highlighted,
        chosen: None,
    };
    let mut ui = TypedUi::new(picker, picking, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    ui.render(&mut scene);
    (ui, scene)
}

fn press(ui: &mut TypedUi<Picker>, scene: &mut Scene, key: Key) {
    ui.handle_event(Event::KeyPressed {
        key,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers: Modifiers::NONE,
    });
    ui.render(scene);
}

#[test]
fn keys_choose_rows_by_walk_and_typeahead() {
    let cases: [(Option<usize>, &[Key], usize); 8] = [
        (Some(3), &[], 3),
        (Some(1), &[Key::ArrowDown], 2),
        (Some(2), &[Key::Home], 0),
        (Some(0), &[Key::End], 3),
        (None, &[Key::Char('b')], 1),
        (None, &[Key::Char('b'), Key::Char('b')], 2),
        (None, &[Key::Char('B'), Key::Char('l')], 2),
        (Some(0), &[Key::Char('d'), Key::Char('c')], 0),
    ];
    for (highlighted, keys, expected) in cases {
        let (mut ui, mut scene) = opened(highlighted);
        for key in keys {
            press(&mut ui, &mut scene, *key);
        }
        press(&mut ui, &mut scene, Key::Enter);
        assert_eq!(
            ui.state().chosen,
            Some((expected, Activation::Keyboard)),
            "{highlighted:?} then {keys:?}"
        );
    }
}

#[test]
fn pressing_a_row_chooses_it_with_the_pointer() {
    let (mut ui, mut scene) = opened(None);
    let first_row = Point::new(40.0, 44.0);
    ui.handle_event(Event::PointerMoved(first_row));
    ui.handle_event(Event::PointerPressed {
        position: first_row,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(&mut scene);
    assert_eq!(ui.state().chosen, Some((0, Activation::Pointer)));
}
