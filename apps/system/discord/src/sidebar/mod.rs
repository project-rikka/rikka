mod account_menu;
mod channels;
mod directs;
mod header;
mod rows;
mod user_panel;

use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::account::Scope;
use crate::chat::Chat;
pub(crate) use rows::kind_symbol;

pub(crate) const WIDTH: f32 = 240.0;

#[derive(State)]
pub(crate) struct Sidebar {
    #[untracked]
    search: TextDocument,
    query: Tracked<String>,
    menu_open: Tracked<bool>,
}

impl Sidebar {
    pub(crate) fn new() -> Sidebar {
        Sidebar {
            search: TextDocument::new(""),
            query: Tracked::new(String::new()),
            menu_open: Tracked::new(false),
        }
    }

    pub(crate) fn search(&self) -> &TextDocument {
        &self.search
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) fn menu_open(&self) -> bool {
        *self.menu_open
    }

    pub(crate) fn searched(&mut self) {
        let query = self.search.text().trim().to_lowercase();
        self.query.set(query);
    }

    pub(crate) fn toggle_menu(&mut self) {
        let open = !*self.menu_open;
        self.menu_open.set(open);
    }

    pub(crate) fn close_menu(&mut self) {
        self.menu_open.set(false);
    }
}

impl std::fmt::Debug for Sidebar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Sidebar")
            .field("query", &*self.query)
            .field("menu_open", &*self.menu_open)
            .finish_non_exhaustive()
    }
}

pub(crate) fn view(chat: &Chat) -> impl View {
    let scope = chat.current().map(|linked| linked.location().scope.clone());
    let header = header::area(scope.as_ref());
    let body = match scope {
        Some(Scope::Space(space)) => component_with(|chat: &Chat| chat, space, channels::view)
            .grow(1.0)
            .any(),
        _ => component(|chat: &Chat| chat, directs::view).grow(1.0).any(),
    };
    column! {
        header;
        body;
        component(|chat: &Chat| chat, crate::voice::panel_view);
        component(|chat: &Chat| chat, user_panel::view);
    }
    .width(WIDTH)
    .fill_height()
    .background(palette().surface)
}
