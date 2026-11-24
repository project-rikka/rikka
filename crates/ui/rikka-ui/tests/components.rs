use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Color, Size};
use rikka_ui::{
    Element, Fetch, Layout, Length, PaintCtx, Rendering, State, TextDocument, TextRun, Tracked,
    TrackedVec, TypedUi, View, Widget, component, component_with, fetch, stats, text,
};

use crate::support::wakes;

const VIEW: Size = Size::new(240.0, 200.0);
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

thread_local! {
    static VIEWED: RefCell<Vec<(&'static str, u32)>> = const { RefCell::new(Vec::new()) };
}

fn viewed(name: &'static str) {
    VIEWED.with(|viewed| {
        let mut viewed = viewed.borrow_mut();
        match viewed
            .iter_mut()
            .find(|(viewed_name, _)| *viewed_name == name)
        {
            Some((_, count)) => *count += 1,
            None => viewed.push((name, 1)),
        }
    });
}

fn views(name: &'static str) -> u32 {
    VIEWED.with(|viewed| {
        viewed
            .borrow()
            .iter()
            .find(|(viewed_name, _)| *viewed_name == name)
            .map_or(0, |(_, count)| *count)
    })
}

struct Pile;

impl Widget for Pile {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().width(Length::FILL)
    }
}

fn pile(children: Vec<Element>) -> Element {
    Element::with_children(Pile, children)
}

struct Harness<S: State> {
    ui: TypedUi<S>,
    scene: Scene,
}

impl<S: State> Harness<S> {
    fn new(ui: TypedUi<S>) -> Self {
        let mut harness = Harness {
            ui,
            scene: Scene::new(),
        };
        harness.ui.set_viewport(VIEW, 1.0);
        harness.frame();
        harness
    }

    fn frame(&mut self) {
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(Instant::now()));
    }

    fn shows(&self, needle: &str) -> bool {
        self.ui.dump().contains(needle)
    }
}

#[derive(Default, State)]
struct Mailbox {
    unread: Tracked<u32>,
    #[state]
    list: MessageList,
}

#[derive(Default, State)]
struct MessageList {
    subjects: TrackedVec<String>,
}

fn mailbox(mailbox: &Mailbox) -> impl View {
    viewed("mailbox");
    pile(vec![
        text(format!("{} unread", *mailbox.unread)).build(),
        component(|mailbox: &Mailbox| &mailbox.list, list),
    ])
}

fn list(list: &MessageList) -> impl View {
    viewed("list");
    pile(
        list.subjects
            .iter()
            .map(|subject| text(subject.clone()).build())
            .collect(),
    )
}

#[test]
fn unchanged_component_is_not_rebuilt() {
    let mut harness = Harness::new(TypedUi::new(Mailbox::default(), mailbox, Theme::dark()));
    assert_eq!((views("mailbox"), views("list")), (1, 1));

    let before = stats::snapshot();
    harness
        .ui
        .start(|mailbox: &mut Mailbox| *mailbox.unread += 3);
    harness.frame();
    let spent = stats::snapshot().since(before);

    assert!(harness.shows("3 unread"));
    assert_eq!(views("mailbox"), 2);
    assert_eq!((spent.components, spent.retained), (0, 1));

    let before = stats::snapshot();
    harness
        .ui
        .start(|list: &mut MessageList| list.subjects.push("hello".to_owned()));
    harness.frame();
    let spent = stats::snapshot().since(before);

    assert!(harness.shows("hello"));
    assert_eq!(views("mailbox"), 2);
    assert_eq!(spent.components, 1);
}

#[derive(Default, State)]
struct Lobby {
    open: Tracked<bool>,
    #[state]
    hall: Hall,
}

#[derive(Default, State)]
struct Hall {
    guests: Tracked<u32>,
}

fn lobby(lobby: &Lobby) -> impl View {
    let mut rows = vec![component(|lobby: &Lobby| &lobby.hall, hall)];
    if *lobby.open {
        rows.push(text("open").build());
    }
    pile(rows)
}

fn hall(_: &Hall) -> impl View {
    component(|hall: &Hall| hall, room)
}

fn room(hall: &Hall) -> impl View {
    text(format!("{} guests", *hall.guests))
}

#[test]
fn component_returning_component_is_kept() {
    let mut harness = Harness::new(TypedUi::new(Lobby::default(), lobby, Theme::dark()));

    harness.ui.start(|lobby: &mut Lobby| lobby.open.set(true));
    harness.frame();
    assert!(harness.shows("open"));

    harness.ui.start(|hall: &mut Hall| hall.guests.set(2));
    harness.frame();
    assert!(harness.shows("2 guests"));
}

#[derive(State)]
struct Inbox {
    letters: TrackedVec<String>,
}

fn inbox(inbox: &Inbox) -> impl View {
    viewed("inbox");
    pile(
        (0..inbox.letters.len())
            .map(|index| component_with(|inbox: &Inbox| inbox, index, envelope).keyed(index))
            .collect(),
    )
}

fn envelope(inbox: &Inbox, index: &usize) -> impl View {
    viewed(["envelope 0", "envelope 1", "envelope 2"][*index]);
    text(inbox.letters.get(*index).cloned().unwrap_or_default())
}

#[test]
fn element_change_rebuilds_only_readers() {
    let letters = ["one", "two", "three"].map(str::to_owned).to_vec();
    let mut harness = Harness::new(TypedUi::new(
        Inbox {
            letters: TrackedVec::from(letters),
        },
        inbox,
        Theme::dark(),
    ));

    let before = stats::snapshot();
    harness
        .ui
        .start(|inbox: &mut Inbox| inbox.letters[1] = "changed".to_owned());
    harness.frame();
    let spent = stats::snapshot().since(before);

    assert!(harness.shows("changed"));
    assert_eq!(views("inbox"), 1);
    assert_eq!(
        (
            views("envelope 0"),
            views("envelope 1"),
            views("envelope 2")
        ),
        (1, 2, 1)
    );
    assert!(spent.elements <= 2, "{} elements", spent.elements);
}

const LANDED: &str = "a component's own fetch";

#[derive(Default, State)]
struct Page {
    title: Tracked<String>,
}

fn page(_: &Page) -> impl View {
    viewed("page");
    pile(vec![
        component(|page: &Page| page, asking),
        component(|page: &Page| page, quiet),
    ])
}

fn asking(_: &Page) -> impl View {
    viewed("asking");
    let answer: Fetch<String> = fetch(LANDED, || async {
        Ok::<String, String>("landed".to_owned())
    });
    text(answer.ready().map_or("waiting", String::as_str).to_owned())
}

fn quiet(page: &Page) -> impl View {
    viewed("quiet");
    text((*page.title).clone())
}

#[test]
fn landed_job_rebuilds_only_requester() {
    let mut ui = TypedUi::new(Page::default(), page, Theme::dark());
    let woken = wakes(&mut ui);
    let mut harness = Harness::new(ui);
    while !harness.shows("landed") {
        assert!(woken.wait(FETCH_TIMEOUT), "fetch did not finish");
        harness.frame();
    }

    assert_eq!((views("page"), views("quiet")), (1, 1));
    assert!(views("asking") >= 2);
}

struct Reading {
    document: TextDocument,
    painted: Rc<RefCell<String>>,
    paints: Rc<Cell<u32>>,
}

impl Widget for Reading {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(20.0))
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        self.paints.set(self.paints.get() + 1);
        let needle = self.document.text();
        ctx.draw_text(
            ctx.bounds(),
            TextRun::new(needle.clone(), 12.0),
            Color::WHITE,
        );
        *self.painted.borrow_mut() = needle;
    }

    fn same(&self, other: &Self) -> bool {
        self.document.same(&other.document) && Rc::ptr_eq(&self.paints, &other.paints)
    }
}

#[derive(Default, State)]
struct Notebook {
    document: TextDocument,
    painted: Rc<RefCell<String>>,
    paints: Rc<Cell<u32>>,
}

fn notebook(notebook: &Notebook) -> impl View {
    Element::new(Reading {
        document: notebook.document.clone(),
        painted: Rc::clone(&notebook.painted),
        paints: Rc::clone(&notebook.paints),
    })
}

#[test]
fn external_document_change_repaints() {
    let mut harness = Harness::new(TypedUi::new(Notebook::default(), notebook, Theme::dark()));
    let paints = harness.ui.state().paints.get();

    harness.ui.state().document.insert("written elsewhere");

    assert!(harness.ui.needs_render());
    harness.frame();
    assert_eq!(
        harness.ui.state().painted.borrow().as_str(),
        "written elsewhere"
    );
    assert_eq!(harness.ui.state().paints.get(), paints + 1);
}

#[derive(Default, State)]
struct Shelf {
    stocked: Tracked<bool>,
    label: Tracked<String>,
}

fn shelf(_: &Shelf) -> impl View {
    pile(vec![
        text("top").build(),
        component(|shelf: &Shelf| shelf, stock),
    ])
}

fn stock(shelf: &Shelf) -> Element {
    match *shelf.stocked {
        true => pile(vec![text(shelf.label.as_str()).build()]),
        false => text("empty").build(),
    }
}

fn restocked(everything: bool) {
    let mut harness = Harness::new(TypedUi::new(Shelf::default(), shelf, Theme::dark()));
    harness.ui.start(|shelf: &mut Shelf| {
        shelf.stocked.set(true);
        shelf.label.set("stocked".to_owned());
    });
    if everything {
        harness.ui.start(|| {});
    }
    harness.frame();
    assert!(harness.shows("stocked"), "{}", harness.ui.dump());

    harness
        .ui
        .start(|shelf: &mut Shelf| shelf.label.set("restocked".to_owned()));
    harness.frame();
    assert!(harness.shows("restocked"), "{}", harness.ui.dump());
}

#[test]
fn component_changing_widget_keeps_position() {
    restocked(false);
    restocked(true);
}
