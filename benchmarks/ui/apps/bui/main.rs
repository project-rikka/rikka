use std::process::ExitCode;
use std::rc::Rc;

use blenderui::prelude::*;
use ui_scene::{
    BACKGROUND, Bounce, CELL_RADIUS, GRID_GAP, Grid, LIST_ROWS, Probe, REMOVE_WIDTH,
    ROW_BACKGROUND, ROW_HEIGHT, ROW_HOVERED, ROW_ID_WIDTH, ROW_TEXT_SIZE, Row, Scenario, TEXT,
    Table, WINDOW_HEIGHT, WINDOW_WIDTH,
};

const LIST_ID: &str = "rows";

enum Content {
    Grid(Grid),
    List { rows: Rc<[Row]>, bounce: Bounce },
}

struct Bench {
    content: Content,
    scenario: Scenario,
    probe: Probe,
    presented: bool,
}

impl Bench {
    fn step(&mut self, window: &mut Window) {
        match &mut self.content {
            Content::Grid(grid) => grid.advance(),
            Content::List { bounce, .. } => {
                if self.scenario.scrolls() {
                    window.set_scroll_offset(LIST_ID, 0.0, bounce.advance());
                }
            }
        }
    }
}

impl Render for Bench {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if self.scenario.animates() {
            self.probe.frame();
            self.step(window);
            window.request_animation_frame();
        } else if !self.presented {
            match window.presented_frames() > 0 {
                true => {
                    self.presented = true;
                    self.probe.frame();
                }
                false => window.request_animation_frame(),
            }
        }
        let content = match &self.content {
            Content::Grid(grid) => grid_view(grid).into_any_element(),
            Content::List { rows, .. } => list_view(Rc::clone(rows)).into_any_element(),
        };
        body().m_0().size_full().child(content)
    }
}

fn grid_view(grid: &Grid) -> Div {
    let layout = grid.layout();
    div()
        .size_full()
        .bg(rgb(BACKGROUND))
        .flex()
        .flex_wrap()
        .content_start()
        .gap(px(GRID_GAP))
        .p(px(GRID_GAP))
        .children(grid.cells().map(|cell| {
            div()
                .w(px(layout.cell_width))
                .h(px(layout.cell_height))
                .bg(rgb(cell.color))
                .rounded(px(CELL_RADIUS))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(layout.text_size))
                .text_color(rgb(TEXT))
                .child(cell.label)
        }))
}

fn list_view(rows: Rc<[Row]>) -> UniformList {
    uniform_list(LIST_ID, rows.len(), move |range, _, _| {
        range
            .map(|index| list_row(&rows[index]))
            .collect::<Vec<_>>()
    })
    .item_height(px(ROW_HEIGHT))
    .w_full()
    .h(px(WINDOW_HEIGHT))
    .bg(rgb(BACKGROUND))
    .text_size(px(ROW_TEXT_SIZE))
    .text_color(rgb(TEXT))
}

fn list_row(row: &Row) -> Div {
    div()
        .h(px(ROW_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .bg(rgb(ROW_BACKGROUND))
        .hover(|style| style.bg(rgb(ROW_HOVERED)))
        .child(div().w(px(ROW_ID_WIDTH)).child(row.id.to_string()))
        .child(div().flex_1().child(row.label.clone()))
        .child(
            div()
                .w(px(REMOVE_WIDTH))
                .flex()
                .justify_center()
                .cursor_pointer()
                .on_click(|_, _, _| {})
                .child("×"),
        )
}

fn main() -> ExitCode {
    let scenario = match Scenario::from_args() {
        Ok(scenario) => scenario,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let probe = Probe::from_env();
    let content = match scenario.grid() {
        Some(grid) => Content::Grid(grid),
        None => Content::List {
            rows: Table::with_rows(LIST_ROWS).rows.into(),
            bounce: Bounce::new(LIST_ROWS),
        },
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
        })),
        ..Default::default()
    };
    let bench = Bench {
        content,
        scenario,
        probe,
        presented: false,
    };
    Application::new().run(move |cx: &mut App| {
        cx.open_window(options, |_, cx| cx.new(|_| bench))
            .expect("the benchmark window opens");
    });
    ExitCode::SUCCESS
}
