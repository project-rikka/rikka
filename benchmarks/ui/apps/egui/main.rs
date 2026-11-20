use std::process::ExitCode;

use eframe::{App, Frame, NativeOptions, Renderer};
use egui::{Ui, ViewportBuilder};
use ui_scene::{Bounce, Grid, LIST_ROWS, Probe, Row, Scenario, Table, WINDOW_HEIGHT, WINDOW_WIDTH};

enum Content {
    Grid(Grid),
    List {
        rows: Vec<Row>,
        bounce: Option<Bounce>,
    },
}

struct Bench {
    content: Content,
    scenario: Scenario,
    probe: Probe,
}

impl Bench {
    fn step(&mut self) {
        match &mut self.content {
            Content::Grid(grid) => grid.advance(),
            Content::List {
                bounce: Some(bounce),
                ..
            } => {
                bounce.advance();
            }
            Content::List { bounce: None, .. } => {}
        }
    }
}

impl App for Bench {
    fn ui(&mut self, ui: &mut Ui, _: &mut Frame) {
        if self.scenario.animates() {
            self.probe.frame();
            self.step();
            ui.ctx().request_repaint();
        }
        match &self.content {
            Content::Grid(grid) => ui_egui_views::grid(ui, grid),
            Content::List { rows, bounce } => {
                ui_egui_views::list(ui, rows, bounce.map(|bounce| bounce.offset()));
            }
        }
    }
}

fn main() -> ExitCode {
    let scenario = match Scenario::from_args() {
        Ok(scenario) => scenario,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let content = match scenario.grid() {
        Some(grid) => Content::Grid(grid),
        None => Content::List {
            rows: Table::with_rows(LIST_ROWS).rows,
            bounce: scenario.scrolls().then(|| Bounce::new(LIST_ROWS)),
        },
    };
    let bench = Bench {
        content,
        scenario,
        probe: Probe::from_env(),
    };
    let options = NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT]),
        renderer: Renderer::Wgpu,
        ..NativeOptions::default()
    };
    match eframe::run_native("ui-bench-egui", options, Box::new(|_| Ok(Box::new(bench)))) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to run egui: {error}");
            ExitCode::FAILURE
        }
    }
}
