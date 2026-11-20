use std::hint::black_box;

use egui::{Context, RawInput};
use ui_scene::{WINDOW_HEIGHT, WINDOW_WIDTH};

use super::{Frontend, Screen};

pub struct Egui {
    context: Context,
    screen: Screen,
}

fn input() -> RawInput {
    RawInput {
        screen_rect: Some(ui_egui_views::full_rect(WINDOW_WIDTH, WINDOW_HEIGHT)),
        ..RawInput::default()
    }
}

impl Egui {
    fn render(&mut self) {
        let screen = &self.screen;
        let output = self.context.run_ui(input(), |ui| match screen {
            Screen::Table(table) => ui_egui_views::table(ui, table),
            Screen::Grid(grid) => ui_egui_views::grid(ui, grid),
        });
        black_box(output.shapes);
    }
}

impl Frontend for Egui {
    type Host = Context;

    fn host() -> Context {
        Context::default()
    }

    fn open(context: Context, screen: Screen) -> Egui {
        let mut frontend = Egui { context, screen };
        frontend.render();
        frontend
    }

    fn update(&mut self, change: impl FnOnce(&mut Screen)) {
        change(&mut self.screen);
        self.render();
    }
}

impl std::fmt::Debug for Egui {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Egui").finish_non_exhaustive()
    }
}
