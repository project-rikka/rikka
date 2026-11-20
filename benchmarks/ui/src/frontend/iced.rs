use iced_core::{Color, Font, Pixels, Size, Theme, mouse, renderer};
use iced_runtime::user_interface::{Cache, UserInterface};
use iced_tiny_skia::Renderer;
use ui_iced_views::{Message, View};
use ui_scene::{WINDOW_HEIGHT, WINDOW_WIDTH};

use super::{Frontend, Screen};

const DEFAULT_TEXT_SIZE: f32 = 16.0;

fn view(screen: &Screen) -> View<'static, Renderer> {
    match screen {
        Screen::Table(table) => ui_iced_views::table(table),
        Screen::Grid(grid) => ui_iced_views::grid(grid),
    }
}

pub struct Iced {
    renderer: Renderer,
    interface: Option<UserInterface<'static, Message, Theme, Renderer>>,
    screen: Screen,
    theme: Theme,
}

impl Iced {
    fn rebuild(&mut self) {
        let cache = self
            .interface
            .take()
            .map_or_else(Cache::default, UserInterface::into_cache);
        self.interface = Some(UserInterface::build(
            view(&self.screen),
            Size::new(WINDOW_WIDTH, WINDOW_HEIGHT),
            cache,
            &mut self.renderer,
        ));
    }

    fn draw(&mut self) {
        let interface = self.interface.as_mut().expect("interface is built");
        interface.draw(
            &mut self.renderer,
            &self.theme,
            &renderer::Style {
                text_color: Color::WHITE,
            },
            mouse::Cursor::Unavailable,
        );
    }
}

impl Frontend for Iced {
    type Host = Renderer;

    fn host() -> Renderer {
        Renderer::new(Font::DEFAULT, Pixels(DEFAULT_TEXT_SIZE))
    }

    fn open(renderer: Renderer, screen: Screen) -> Iced {
        let mut frontend = Iced {
            renderer,
            interface: None,
            screen,
            theme: Theme::Dark,
        };
        frontend.rebuild();
        frontend.draw();
        frontend
    }

    fn update(&mut self, change: impl FnOnce(&mut Screen)) {
        change(&mut self.screen);
        self.rebuild();
        self.draw();
    }
}

impl std::fmt::Debug for Iced {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Iced").finish_non_exhaustive()
    }
}
