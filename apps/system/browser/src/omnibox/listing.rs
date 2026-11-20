use std::ops::Range;
use std::sync::Arc;

use browser_model::Favicon;
use rikka_app::prelude::*;
use rikka_omnibox::{Action, Match, MatchKind, Ranked, SearchEngines, Surface, UrlDisplay};

use crate::model::Model;
use crate::omnibox::target::{Act, Target};
use browser_model::Slot;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Look {
    Favicon(Favicon),
    Stored(String),
    Symbol(MaterialSymbol),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) kind: MatchKind,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) title_marks: Vec<Range<usize>>,
    pub(crate) detail_marks: Vec<Range<usize>>,
    pub(crate) look: Look,
    pub(crate) switches: bool,
    pub(crate) removable: bool,
    pub(crate) fill: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Listing {
    pub(crate) surface: Surface,
    pub(crate) rows: Arc<[Row]>,
    pub(crate) selected: Option<usize>,
    pub(crate) hovered: Option<usize>,
}

impl Listing {
    pub(crate) fn new(
        surface: Surface,
        ranked: &Ranked<Target>,
        model: &Model,
        engines: &SearchEngines,
    ) -> Listing {
        let rows: Arc<[Row]> = ranked
            .matches
            .iter()
            .map(|ranked_match| row(ranked_match, model, engines))
            .collect();
        let selected = (!rows.is_empty()).then_some(0);
        Listing {
            surface,
            rows,
            selected,
            hovered: None,
        }
    }

    pub(crate) fn step(&mut self, by: isize) -> Option<usize> {
        let count = self.rows.len();
        if count == 0 {
            return None;
        }
        let next = match self.selected {
            Some(at) => (at as isize + by).rem_euclid(count as isize) as usize,
            None if by < 0 => count - 1,
            None => 0,
        };
        self.selected = Some(next);
        Some(next)
    }
}

fn row(found: &Match<Target>, model: &Model, engines: &SearchEngines) -> Row {
    let (look, switches, fill) = match &found.action {
        Action::Navigate(url) => (
            Look::Stored(url.as_str().to_owned()),
            false,
            UrlDisplay::steady(url).into_text(),
        ),
        Action::Search(search) => {
            let fill = match search.by_keyword() {
                true => engines.get(search.engine()).map_or_else(
                    || search.terms().to_owned(),
                    |engine| format!("{} {}", engine.keyword(), search.terms()),
                ),
                false => search.terms().to_owned(),
            };
            (Look::Symbol(MaterialSymbol::SEARCH), false, fill)
        }
        Action::SwitchToTab(target) | Action::RestoreArchived(target) => (
            look_of(*target, found.kind, model),
            matches!(target, Target::Tab(_)),
            found.url.clone(),
        ),
        Action::RunCommand(target) => (
            look_of(*target, found.kind, model),
            false,
            found.title.clone(),
        ),
    };
    let detail = match &found.action {
        Action::Search(search) => engines
            .get(search.engine())
            .map(|engine| tr!("browser.omnibox.search-with", engine = engine.name()).to_string())
            .unwrap_or_default(),
        _ if found.url == found.title => String::new(),
        _ => found.url.clone(),
    };
    let detail_marks = match detail == found.url {
        true => found.url_highlights.clone(),
        false => Vec::new(),
    };
    let title = match found.title.is_empty() {
        true => found.url.clone(),
        false => found.title.clone(),
    };
    let title_marks = match found.title.is_empty() {
        true => found.url_highlights.clone(),
        false => found.title_highlights.clone(),
    };
    Row {
        kind: found.kind,
        title,
        detail,
        title_marks,
        detail_marks,
        look,
        switches,
        removable: found.kind == MatchKind::History,
        fill,
    }
}

fn look_of(target: Target, kind: MatchKind, model: &Model) -> Look {
    let favicon = match target {
        Target::Tab(tab) => model
            .tab(tab)
            .and_then(|tab_entry| (*tab_entry.meta.favicon).clone()),
        Target::Slot(Slot::Pinned(pin), _) => model
            .pinned
            .pin(pin)
            .and_then(|(_, pin)| pin.saved.favicon.clone()),
        Target::Slot(Slot::Favorite(favorite), _) => model
            .favorites
            .find(favorite)
            .and_then(|(_, favorite)| favorite.saved.favicon.clone()),
        Target::Archived(_) | Target::Act(_) => None,
    };
    if let Some(favicon) = favicon {
        return Look::Favicon(favicon);
    }
    Look::Symbol(match (target, kind) {
        (Target::Act(act), _) => act_symbol(act),
        (_, MatchKind::Pinned) => MaterialSymbol::PUSH_PIN,
        (_, MatchKind::Favorite) => MaterialSymbol::STAR,
        (_, MatchKind::Archived) => MaterialSymbol::INVENTORY_2,
        _ => MaterialSymbol::TAB,
    })
}

pub(crate) fn act_symbol(act: Act) -> MaterialSymbol {
    use crate::command::Command;
    match act {
        Act::NewSpace => MaterialSymbol::ADD_BOX,
        Act::Space(_) => MaterialSymbol::WORKSPACES,
        Act::Archive => MaterialSymbol::INVENTORY_2,
        Act::CopyAddress => MaterialSymbol::CONTENT_COPY,
        Act::Run(command) => match command {
            Command::NewTab => MaterialSymbol::ADD,
            Command::NewWindow => MaterialSymbol::DESKTOP_WINDOWS,
            Command::PrivateWindow => MaterialSymbol::VISIBILITY_OFF,
            Command::Settings => MaterialSymbol::SETTINGS,
            Command::History => MaterialSymbol::HISTORY,
            Command::ClearData => MaterialSymbol::DELETE,
            Command::ZoomIn | Command::ZoomOut | Command::ZoomReset => MaterialSymbol::ZOOM_IN,
            Command::TogglePin => MaterialSymbol::PUSH_PIN,
            Command::Reload | Command::HardReload => MaterialSymbol::REFRESH,
            Command::ReaderMode => MaterialSymbol::CHROME_READER_MODE,
            Command::ToggleSidebar => MaterialSymbol::VIEW_SIDEBAR,
            Command::ToggleSplit => MaterialSymbol::VERTICAL_SPLIT,
            Command::Find => MaterialSymbol::FIND_IN_PAGE,
            Command::ViewSource => MaterialSymbol::CODE,
            Command::Translate => MaterialSymbol::TRANSLATE,
            Command::Downloads => MaterialSymbol::DOWNLOAD,
            Command::Fullscreen => MaterialSymbol::FULLSCREEN,
            _ => MaterialSymbol::BOLT,
        },
    }
}
