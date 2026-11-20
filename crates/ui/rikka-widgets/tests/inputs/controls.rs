use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::{Theme, builtin_themes};
use rikka_types::{Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::{column, *};

const VIEW: Size = Size::new(400.0, 220.0);

fn center() -> Point {
    Point::new(VIEW.width / 2.0, VIEW.height / 2.0)
}

const SETTLED: Duration = Duration::from_secs(5);

const TILE_CONTENT: f32 = 90.0;
const TILE_WIDTH: f32 = 184.0;

const LIMIT: usize = 3;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Button,
    Switch,
    Checkbox,
    Radio,
    Field,
    Filter,
    Secret,
    Limited,
    Slider,
    Segmented,
    Passive,
    Tabs,
    FullTabs,
    Dropdown,
    Progress,
    Indeterminate,
    Spinner,
    Nav,
    Menu,
    List,
    Tile,
}

const EVERY_KIND: [Kind; 21] = [
    Kind::Button,
    Kind::Switch,
    Kind::Checkbox,
    Kind::Radio,
    Kind::Field,
    Kind::Filter,
    Kind::Secret,
    Kind::Limited,
    Kind::Slider,
    Kind::Segmented,
    Kind::Passive,
    Kind::Tabs,
    Kind::FullTabs,
    Kind::Dropdown,
    Kind::Progress,
    Kind::Indeterminate,
    Kind::Spinner,
    Kind::Nav,
    Kind::Menu,
    Kind::List,
    Kind::Tile,
];

#[derive(Clone, Debug, PartialEq)]
enum Recorded {
    Pressed,
    Toggled(bool),
    Selected,
    Changed(f32),
    Released(f32),
    Text,
    Index(usize),
    Closed(usize),
}

#[derive(State)]
struct Probe {
    kind: Kind,
    log: Vec<Recorded>,
    on: bool,
    value: f32,
    text: TextDocument,
    index: usize,
    highlight: Option<usize>,
}

impl Probe {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            log: Vec::new(),
            on: false,
            value: 0.5,
            text: TextDocument::new("あい"),
            index: 0,
            highlight: None,
        }
    }

    fn pressed(&mut self) {
        self.log.push(Recorded::Pressed);
    }

    fn toggled(&mut self, on: bool) {
        self.on = on;
        self.log.push(Recorded::Toggled(on));
    }

    fn selected(&mut self) {
        self.on = true;
        self.log.push(Recorded::Selected);
    }

    fn changed(&mut self, value: f32) {
        self.value = value;
        self.log.push(Recorded::Changed(value));
    }

    fn released(&mut self, value: f32) {
        self.value = value;
        self.log.push(Recorded::Released(value));
    }

    fn edited(&mut self) {
        self.log.push(Recorded::Text);
    }

    fn chose(&mut self, index: usize) {
        self.index = index;
        self.log.push(Recorded::Index(index));
    }

    fn closed(&mut self, index: usize) {
        self.log.push(Recorded::Closed(index));
    }

    fn highlighted(&mut self, row: Option<usize>) {
        self.highlight = row;
    }

    fn activated(&mut self, _row: usize) {
        self.pressed();
    }

    fn control(&self) -> Element {
        match self.kind {
            Kind::Button => button("保存").on_press(Probe::pressed).build(),
            Kind::Tile => container(
                column! {
                    spacer().height(Length::px(TILE_CONTENT));
                    text("Nord").label()
                }
                .gap(Space::SNUG),
            )
            .padding(Space::SNUG)
            .radius(14.0)
            .width(Length::px(TILE_WIDTH))
            .on_press(Probe::pressed)
            .build(),
            Kind::Switch => switch(self.on).on_toggle(Probe::toggled).build(),
            Kind::Checkbox => checkbox(self.on)
                .label("同意する")
                .on_toggle(Probe::toggled)
                .build(),
            Kind::Radio => radio(self.on)
                .label("Pro")
                .on_select(Probe::selected)
                .build(),
            Kind::Field => text_field(&self.text)
                .placeholder("you@example.com")
                .on_change(Probe::edited)
                .on_submit(Probe::pressed)
                .build(),
            Kind::Filter => text_field(&self.text)
                .placeholder("検索")
                .leaves_arrows(true)
                .on_change(Probe::edited)
                .on_submit(Probe::pressed)
                .build(),
            Kind::Secret => text_field(&self.text)
                .secret(!self.on)
                .on_change(Probe::edited)
                .build(),
            Kind::Limited => text_field(&self.text)
                .max_len(LIMIT)
                .on_change(Probe::edited)
                .build(),
            Kind::Slider => slider(self.value)
                .range(0.0..=100.0)
                .ticks(5)
                .on_change(Probe::changed)
                .on_commit(Probe::released)
                .build(),
            Kind::Segmented => segmented(self.index)
                .segments(["すべて", "未読", "フラグ"])
                .on_select(Probe::chose)
                .build(),
            Kind::Passive => segmented(self.index)
                .segments(["すべて", "未読", "フラグ"])
                .focusless()
                .on_select(Probe::chose)
                .build(),
            Kind::Tabs => tabs(self.index)
                .items([
                    tab("スタート").icon(MaterialSymbol::HOME).closable(false),
                    tab("gallery.rs").icon(MaterialSymbol::DATA_OBJECT),
                    tab("CLAUDE.md"),
                ])
                .on_select(Probe::chose)
                .on_close(Probe::closed)
                .on_new(Probe::pressed)
                .build(),
            Kind::FullTabs => tabs(self.index)
                .items((0..20).map(|index| tab(format!("tab {index}"))))
                .on_select(Probe::chose)
                .on_close(Probe::closed)
                .on_new(Probe::pressed)
                .build(),
            Kind::Dropdown => dropdown(self.index)
                .options(["ケーキ", "パイ", "プリン"])
                .placeholder("選んでください")
                .open(self.on)
                .on_toggle(Probe::toggled)
                .on_select(Probe::chose)
                .build(),
            Kind::Progress => progress(self.value).build(),
            Kind::Indeterminate => progress_indeterminate().build(),
            Kind::Spinner => spinner().build(),
            Kind::Nav => nav_list(self.index)
                .items([
                    nav_header("受信"),
                    nav_item("トレイ").icon(MaterialSymbol::INBOX),
                    nav_item("送信済み"),
                ])
                .on_select(Probe::chose)
                .build(),
            Kind::Menu => menu()
                .items([
                    menu_item("コピー").shortcut("Ctrl+C"),
                    menu_separator(),
                    menu_header("整理"),
                    menu_item("移動先").submenu(true),
                    menu_item("削除")
                        .icon(MaterialSymbol::DELETE)
                        .enabled(false),
                ])
                .on_activate(Probe::chose)
                .on_highlight(Probe::highlighted)
                .on_dismiss(Probe::pressed)
                .build(),
            Kind::List => list([
                list_header("crates"),
                list_item("rikka-ui")
                    .icon(MaterialSymbol::FOLDER)
                    .expanded(false)
                    .on_expand(Probe::selected),
                list_item("Cargo.toml").trailing("2 KB"),
            ])
            .selected(self.index)
            .on_select(Probe::chose)
            .on_activate(Probe::activated)
            .build(),
        }
    }
}

fn view(probe: &Probe) -> Element {
    column! { probe.control() }
        .align_items(Align::Center)
        .justify(Justify::Center)
        .padding(Space::ROOMY)
        .fill()
        .build()
}

fn probe(kind: Kind, theme: Theme) -> (TypedUi<Probe>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(Probe::new(kind), view, theme);
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);
    (ui, scene)
}

fn click<S: State>(ui: &mut TypedUi<S>, at: Point) {
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
}

fn settle(ui: &mut TypedUi<Probe>, scene: &mut Scene) {
    let now = Instant::now();
    ui.render(Rendering::new(scene).with_time(now));
    ui.render(Rendering::new(scene).with_time(now + SETTLED));
}

fn press(ui: &mut TypedUi<Probe>, key: Key) {
    ui.handle_event(Event::KeyPressed {
        key,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers: Modifiers::default(),
    });
}

const CONTROL: Modifiers = Modifiers {
    control: true,
    ..Modifiers::NONE
};

fn chord(ui: &mut TypedUi<Probe>, key: Key, modifiers: Modifiers) {
    ui.handle_event(Event::KeyPressed {
        key,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers,
    });
}

#[test]
fn controls_paint_under_builtin_themes() {
    for spec in builtin_themes() {
        for kind in EVERY_KIND {
            let (mut ui, mut scene) = probe(kind, Theme::from_spec(spec));
            ui.update(Probe::pressed);
            ui.render(&mut scene);
            assert!(
                scene.background() == Theme::from_spec(spec).palette().background,
                "{kind:?} on `{}` did not sync the theme background",
                spec.slug
            );
        }
    }
}

#[test]
fn click_activates_button() {
    let (mut ui, mut scene) = probe(Kind::Button, Theme::dark());
    click(&mut ui, center());
    assert_eq!(ui.state().log, vec![Recorded::Pressed]);

    ui.render(&mut scene);
    click(&mut ui, Point::new(4.0, 4.0));
    assert_eq!(ui.state().log.len(), 1);
}

#[test]
fn keys_edit_focused_field() {
    let (mut ui, mut scene) = probe(Kind::Field, Theme::dark());
    click(&mut ui, center());
    ui.render(&mut scene);

    press(&mut ui, Key::Char('う'));
    assert_eq!(ui.state().text.text(), "あいう");

    press(&mut ui, Key::ArrowLeft);
    press(&mut ui, Key::Backspace);
    assert_eq!(ui.state().text.text(), "あう");

    press(&mut ui, Key::Home);
    press(&mut ui, Key::Delete);
    assert_eq!(ui.state().text.text(), "う");

    press(&mut ui, Key::Enter);
    assert_eq!(ui.state().log.last(), Some(&Recorded::Pressed));
}

#[test]
fn deleting_overflowing_text_keeps_end_at_field_edge() {
    let (mut ui, mut scene) = probe(Kind::Field, Theme::dark());
    click(&mut ui, center());
    ui.render(&mut scene);
    press(&mut ui, Key::End);
    for digit in "0123456789".chars().cycle().take(80) {
        press(&mut ui, Key::Char(digit));
    }
    settle(&mut ui, &mut scene);
    let typed = ui.caret().expect("caret after typing");

    for _ in 0..4 {
        press(&mut ui, Key::Backspace);
    }
    settle(&mut ui, &mut scene);
    let deleted = ui.caret().expect("caret after deleting");
    assert!(
        (deleted.x() - typed.x()).abs() < 0.5,
        "text end left field edge: caret {} after deleting, {} after typing",
        deleted.x(),
        typed.x()
    );
}

#[test]
fn secret_field_hides_and_blocks_copy() {
    let (mut ui, mut scene) = probe(Kind::Secret, Theme::dark());
    click(&mut ui, center());
    ui.render(&mut scene);

    chord(&mut ui, Key::Char('a'), CONTROL);
    chord(&mut ui, Key::Char('x'), CONTROL);
    assert_eq!(ui.state().text.text(), "あい", "secret text copied or cut");

    ui.handle_event(Event::Preedit {
        text: "か".to_owned(),
        cursor: None,
    });
    settle(&mut ui, &mut scene);
    assert!(
        ui.caret().is_none(),
        "secret field exposed caret to input method"
    );

    let debug = format!("{:?}", ui.state().text);
    assert!(!debug.contains('あ'), "secret text reported in plain");
}

#[test]
fn max_length_field_rejects_overflow() {
    let (mut ui, mut scene) = probe(Kind::Limited, Theme::dark());
    click(&mut ui, center());
    ui.render(&mut scene);

    press(&mut ui, Key::Char('う'));
    assert_eq!(ui.state().text.text(), "あいう");

    press(&mut ui, Key::Char('え'));
    assert_eq!(
        ui.state().text.text(),
        "あいう",
        "field not limited to {LIMIT} characters"
    );

    chord(&mut ui, Key::Char('a'), CONTROL);
    ui.handle_event(Event::Commit("かきくけこ".to_owned()));
    assert_eq!(
        ui.state().text.text(),
        "かきく",
        "overflowing commit not truncated"
    );
}

#[test]
fn tab_moves_focus_between_controls() {
    #[derive(State)]
    struct Form {
        first: bool,
        log: Vec<Recorded>,
    }

    fn view(form: &Form) -> Element {
        column! {
            switch(form.first).on_toggle(|form: &mut Form, on: bool| {
                form.first = on;
                form.log.push(Recorded::Toggled(on));
            });
            button("次へ").on_press(|| {});
        }
        .gap(Space::SNUG)
        .padding(Space::ROOMY)
        .build()
    }

    let mut scene = Scene::new();
    let mut ui = TypedUi::new(
        Form {
            first: false,
            log: Vec::new(),
        },
        view,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);

    ui.handle_event(Event::KeyPressed {
        key: Key::Tab,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers: Modifiers::default(),
    });
    ui.handle_event(Event::KeyPressed {
        key: Key::Space,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers: Modifiers::default(),
    });
    assert_eq!(ui.state().log, vec![Recorded::Toggled(true)]);
}

#[test]
fn disabling_slider_mid_drag_releases_pointer() {
    #[derive(State)]
    struct Gauge {
        value: f32,
        presses: usize,
    }

    fn view(gauge: &Gauge) -> Element {
        column! {
            slider(gauge.value)
                .enabled(gauge.value > 0.0)
                .on_change(|gauge: &mut Gauge, value: f32| gauge.value = value)
                .fill_width()
                .height(Length::px(40.0));
            button("次へ")
                .on_press(|gauge: &mut Gauge| gauge.presses += 1)
                .fill_width()
                .grow(1.0);
        }
        .fill()
        .build()
    }

    let mut scene = Scene::new();
    let mut ui = TypedUi::new(
        Gauge {
            value: 0.5,
            presses: 0,
        },
        view,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    ui.render(&mut scene);

    let knob = Point::new(VIEW.width / 2.0, 20.0);
    let start = Point::new(0.0, 20.0);
    ui.handle_event(Event::PointerMoved(knob));
    ui.handle_event(Event::PointerPressed {
        position: knob,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.handle_event(Event::PointerMoved(start));
    assert_eq!(ui.state().value, 0.0);
    ui.render(&mut scene);
    ui.handle_event(Event::PointerReleased {
        position: start,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.render(&mut scene);

    click(&mut ui, Point::new(VIEW.width / 2.0, VIEW.height - 40.0));
    assert_eq!(ui.state().presses, 1, "button ignored next click");
}
