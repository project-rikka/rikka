use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(500.0, 600.0);
const LINES: usize = 8;
const MOST: f32 = 120.0;
const HEADING: f32 = 10.0;
const ROWS: usize = 30;
const ROW: f32 = 150.0;
const FITTED: usize = 3;

struct Probe {
    height: Rc<Cell<f32>>,
}

impl Widget for Probe {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::HUG)
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        self.height.set(ctx.bounds().height());
    }
}

#[derive(Clone, Copy, Debug)]
enum Host {
    Column,
    Scrollable,
    LazyColumn,
}

#[derive(Clone, State)]
struct Page {
    code: TextDocument,
    wrap: bool,
    host: Host,
    height: Rc<Cell<f32>>,
}

fn fitted(page: &Page) -> Element {
    let editor = text_editor(&page.code)
        .read_only(true)
        .gutter(false)
        .wrap(page.wrap)
        .fits_text(0.0..=MOST)
        .fill_width()
        .build();
    let heading = container(spacer())
        .height(Length::px(HEADING))
        .fill_width()
        .build();
    Element::with_children(
        Probe {
            height: Rc::clone(&page.height),
        },
        vec![heading, editor],
    )
}

fn row(page: &Page, index: usize) -> Element {
    match index == FITTED {
        true => fitted(page),
        false => container(spacer())
            .height(Length::px(ROW))
            .fill_width()
            .build(),
    }
}

fn page_view(page: &Page) -> Element {
    match page.host {
        Host::Column => column_of([fitted(page)]).fill().build(),
        Host::Scrollable => {
            let rows = column_of((0..ROWS).map(|index| row(page, index))).fill_width();
            scrollable(rows).fill().build()
        }
        Host::LazyColumn => {
            let page = page.clone();
            lazy_column(ROWS, move |slot| row(&page, slot.index))
                .estimate(Length::px(ROW))
                .fill()
                .build()
        }
    }
}

fn settled_height(host: Host, wrap: bool, text: &str) -> f32 {
    let height = Rc::new(Cell::new(0.0));
    let mut ui = TypedUi::new(
        Page {
            code: TextDocument::new(text),
            wrap,
            host,
            height: Rc::clone(&height),
        },
        page_view,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let mut now = Instant::now();
    for _ in 0..30 {
        now += Duration::from_millis(16);
        ui.request_frame();
        ui.render(Rendering::new(&mut scene).with_time(now));
    }
    height.get()
}

#[test]
fn fitted_editor_keeps_height_in_scroller() {
    let line = "wide ".repeat(200);
    let text = [line.as_str(); LINES].join("\n");
    let collapsed: Vec<(Host, bool, f32)> = [Host::Column, Host::Scrollable, Host::LazyColumn]
        .into_iter()
        .flat_map(|host| [(host, true), (host, false)])
        .map(|(host, wrap)| (host, wrap, settled_height(host, wrap, &text)))
        .filter(|(_, _, height)| *height != HEADING + MOST)
        .collect();
    assert!(
        collapsed.is_empty(),
        "fitted editors not {} tall: {collapsed:?}",
        HEADING + MOST
    );
}
