use std::time::Instant;

use rikka_renderer::{Scene, SceneFrame};
use rikka_theme::Theme;
use rikka_ui::{Rendering, Size, State, TypedUi, View};
use ui_scene::{FRAME_INTERVAL, WINDOW_HEIGHT, WINDOW_WIDTH};

use super::{Frontend, Screen};

#[derive(State)]
struct Shown {
    screen: Screen,
}

fn shown(shown: &Shown) -> impl View {
    match &shown.screen {
        Screen::Table(table) => ui_rikka_views::table(table).any(),
        Screen::Grid(grid) => ui_rikka_views::grid(grid).any(),
    }
}

pub struct Rikka {
    ui: TypedUi<Shown>,
    scene: Scene,
    frame: SceneFrame,
    now: Instant,
}

impl Rikka {
    fn render(&mut self) {
        self.now += FRAME_INTERVAL;
        self.ui
            .render(Rendering::new(&mut self.scene).with_time(self.now));
        self.scene.take_frame(&mut self.frame);
    }
}

impl Frontend for Rikka {
    type Host = ();

    fn host() {}

    fn open((): (), screen: Screen) -> Rikka {
        let mut ui = TypedUi::new(Shown { screen }, shown, Theme::dark());
        ui.set_viewport(Size::new(WINDOW_WIDTH, WINDOW_HEIGHT), 1.0);
        let mut frontend = Rikka {
            ui,
            scene: Scene::new(),
            frame: SceneFrame::default(),
            now: Instant::now(),
        };
        frontend.render();
        frontend
    }

    fn update(&mut self, change: impl FnOnce(&mut Screen)) {
        self.ui.update(|shown| change(&mut shown.screen));
        self.render();
    }
}

impl std::fmt::Debug for Rikka {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Rikka").finish_non_exhaustive()
    }
}
