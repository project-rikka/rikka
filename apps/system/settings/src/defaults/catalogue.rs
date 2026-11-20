use std::path::Path;
use std::sync::{Arc, OnceLock};

use rikka_app::prelude::*;
use rikka_icons::Icons;
use rikka_mime::{Database, MimeType};
use rikka_settings::Store;
use rikka_settings::session::appearance::{self, ICON_THEME, ICON_THEME_DEFAULT};
use rikka_xdg::{Associations, DesktopEntry, MimeApps, Terminals};

const ICON_SIZE: u32 = 32;
const SCHEME: &str = "x-scheme-handler";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    Only,
    Kind(&'static str),
    Also(&'static [&'static str]),
}

#[derive(Debug)]
pub(crate) struct Category {
    pub(crate) slug: &'static str,
    pub(crate) symbol: MaterialSymbol,
    pub(crate) types: &'static [&'static str],
    pub(crate) family: Family,
}

pub(crate) const CATEGORIES: [Category; 10] = [
    Category {
        slug: "web",
        symbol: MaterialSymbol::PUBLIC,
        types: &[
            "x-scheme-handler/https",
            "x-scheme-handler/http",
            "text/html",
            "application/xhtml+xml",
        ],
        family: Family::Only,
    },
    Category {
        slug: "mail",
        symbol: MaterialSymbol::MAIL,
        types: &["x-scheme-handler/mailto"],
        family: Family::Also(&["message/rfc822"]),
    },
    Category {
        slug: "calendar",
        symbol: MaterialSymbol::CALENDAR_MONTH,
        types: &["text/calendar"],
        family: Family::Also(&["x-scheme-handler/webcal"]),
    },
    Category {
        slug: "music",
        symbol: MaterialSymbol::MUSIC_NOTE,
        types: &["audio/mpeg"],
        family: Family::Kind("audio/"),
    },
    Category {
        slug: "video",
        symbol: MaterialSymbol::MOVIE,
        types: &["video/mp4"],
        family: Family::Kind("video/"),
    },
    Category {
        slug: "photos",
        symbol: MaterialSymbol::IMAGE,
        types: &["image/jpeg"],
        family: Family::Kind("image/"),
    },
    Category {
        slug: "documents",
        symbol: MaterialSymbol::PICTURE_AS_PDF,
        types: &["application/pdf"],
        family: Family::Also(&[
            "application/postscript",
            "application/epub+zip",
            "image/vnd.djvu",
        ]),
    },
    Category {
        slug: "text",
        symbol: MaterialSymbol::DESCRIPTION,
        types: &["text/plain"],
        family: Family::Kind("text/"),
    },
    Category {
        slug: "files",
        symbol: MaterialSymbol::FOLDER,
        types: &["inode/directory"],
        family: Family::Only,
    },
    Category {
        slug: "archives",
        symbol: MaterialSymbol::FOLDER_ZIP,
        types: &["application/zip"],
        family: Family::Also(&[
            "application/x-tar",
            "application/x-compressed-tar",
            "application/x-bzip2-compressed-tar",
            "application/x-xz-compressed-tar",
            "application/x-zstd-compressed-tar",
            "application/gzip",
            "application/x-bzip2",
            "application/x-xz",
            "application/zstd",
            "application/x-7z-compressed",
            "application/vnd.rar",
            "application/x-rar",
        ]),
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct App {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) icon: Option<Arc<Path>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Choice {
    pub(crate) current: Option<String>,
    pub(crate) offered: Vec<App>,
    pub(crate) chosen: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Kind {
    pub(crate) mime: MimeType,
    pub(crate) comment: Option<String>,
    pub(crate) choice: Choice,
    search: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Group {
    pub(crate) types: Vec<MimeType>,
    pub(crate) choice: Choice,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MimeInfo(Arc<OnceLock<Database>>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Catalogue {
    pub(crate) groups: Vec<Group>,
    pub(crate) files: Vec<Kind>,
    pub(crate) links: Vec<Kind>,
    pub(crate) terminal: Choice,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Category { app: String, types: Vec<MimeType> },
    Kind { app: String, mime: MimeType },
    Terminal(String),
    Forget(Vec<MimeType>),
    ForgetTerminal,
    ForgetAll(Vec<MimeType>),
}

impl Catalogue {
    pub(crate) fn read(mime_info: &MimeInfo) -> Result<Catalogue, String> {
        let apps = MimeApps::session().ok_or_else(no_home)?;
        let associations = apps.read();
        let icons = Icons::new(&icon_theme());
        let database = mime_info.database();
        database.set_language(&rikka_i18n::current_locale().to_string());
        let app = |entry: &DesktopEntry| App {
            id: entry.id.clone(),
            name: entry.name.clone(),
            icon: entry
                .icon
                .as_deref()
                .and_then(|name| icons.lookup(name, ICON_SIZE, 1)),
        };
        let choice = |mime: &MimeType| {
            let ancestry = database.ancestry(mime);
            Choice {
                current: associations
                    .default_for_any(&ancestry)
                    .map(|entry| entry.id.clone()),
                offered: associations
                    .handlers_for_any(&ancestry)
                    .into_iter()
                    .map(app)
                    .collect(),
                chosen: associations.is_chosen(mime),
            }
        };
        let known = associations.types();
        let groups = CATEGORIES
            .iter()
            .map(|about| {
                let types = members(about, &known);
                let mut choice = choice(&types[0]);
                choice.chosen = types.iter().any(|mime| associations.is_chosen(mime));
                Group { types, choice }
            })
            .collect();
        let (links, files): (Vec<&MimeType>, Vec<&MimeType>) =
            known.iter().partition(|mime| mime.kind() == SCHEME);
        let kind = |mime: &&MimeType| {
            let comment = database.describe(mime);
            Kind {
                search: [mime.as_str(), comment.as_deref().unwrap_or_default()]
                    .join("\n")
                    .to_lowercase(),
                mime: (*mime).clone(),
                comment,
                choice: choice(mime),
            }
        };
        let terminals = Terminals::session().ok_or_else(no_home)?;
        let terminal = Choice {
            current: terminals
                .preferred()
                .map(|terminal| terminal.entry().id.clone()),
            offered: terminals.installed().iter().map(app).collect(),
            chosen: terminals.is_chosen(),
        };
        Ok(Catalogue {
            groups,
            files: files.iter().map(kind).collect(),
            links: links.iter().map(kind).collect(),
            terminal,
        })
    }

    pub(crate) fn category_change(&self, group: usize, app: &str) -> Option<Change> {
        let group = self.groups.get(group)?;
        Some(Change::Category {
            app: app.to_owned(),
            types: group.types.clone(),
        })
    }

    pub(crate) fn chosen_types(&self) -> Vec<MimeType> {
        let groups = self
            .groups
            .iter()
            .filter(|group| group.choice.chosen)
            .flat_map(|group| group.types.iter().cloned());
        let kinds = self
            .files
            .iter()
            .chain(&self.links)
            .filter(|kind| kind.choice.chosen)
            .map(|kind| kind.mime.clone());
        let mut types: Vec<MimeType> = groups.chain(kinds).collect();
        types.sort_unstable();
        types.dedup();
        types
    }

    pub(crate) fn chooses_any(&self) -> bool {
        self.terminal.chosen || !self.chosen_types().is_empty()
    }
}

impl Change {
    pub(crate) fn apply(self) -> Result<(), String> {
        let apps = MimeApps::session().ok_or_else(no_home)?;
        let terminals = Terminals::session().ok_or_else(no_home)?;
        match self {
            Change::Category { app, types } => {
                let associations = apps.read();
                let opened: Vec<MimeType> = types
                    .into_iter()
                    .enumerate()
                    .filter(|(nth, mime)| *nth == 0 || opens(&associations, &app, mime))
                    .map(|(_, mime)| mime)
                    .collect();
                apps.set_default(&app, &opened)
            }
            Change::Kind { app, mime } => apps.set_default(&app, &[mime]),
            Change::Terminal(app) => terminals.prefer(&app),
            Change::Forget(types) => apps.forget_default(&types),
            Change::ForgetTerminal => terminals.forget_choice(),
            Change::ForgetAll(types) => apps
                .forget_default(&types)
                .and_then(|()| terminals.forget_choice()),
        }
        .map_err(|error| error.to_string())
    }
}

impl Choice {
    pub(crate) fn current(&self) -> Option<&App> {
        let current = self.current.as_deref()?;
        self.offered.iter().find(|app| app.id == current)
    }

    pub(crate) fn position(&self) -> Option<usize> {
        let current = self.current.as_deref()?;
        self.offered.iter().position(|app| app.id == current)
    }
}

impl MimeInfo {
    fn database(&self) -> &Database {
        self.0.get_or_init(Database::load)
    }
}

impl Kind {
    pub(crate) fn title(&self) -> String {
        match self.mime.kind() == SCHEME {
            true => format!("{}:", self.mime.subtype()),
            false => self
                .comment
                .clone()
                .unwrap_or_else(|| self.mime.to_string()),
        }
    }

    pub(crate) fn matches(&self, query: &str) -> bool {
        self.search.contains(query)
            || self
                .choice
                .current()
                .is_some_and(|app| app.name.to_lowercase().contains(query))
    }
}

pub(crate) fn icon_of(app: &App) -> IconSource {
    match &app.icon {
        Some(path) => IconSource::from(Arc::clone(path)),
        None => IconSource::from(MaterialSymbol::APPS),
    }
}

fn members(category: &Category, known: &[MimeType]) -> Vec<MimeType> {
    let claimed = |mime: &MimeType| {
        CATEGORIES
            .iter()
            .filter(|other| !std::ptr::eq(*other, category))
            .any(|other| other.types.contains(&mime.as_str()))
    };
    let family: Vec<MimeType> = match category.family {
        Family::Only => Vec::new(),
        Family::Kind(kind) => known
            .iter()
            .filter(|mime| mime.kind() == kind)
            .cloned()
            .collect(),
        Family::Also(listed) => listed.iter().copied().map(MimeType::well_known).collect(),
    };
    let mut types: Vec<MimeType> = category
        .types
        .iter()
        .copied()
        .map(MimeType::well_known)
        .collect();
    for mime in family {
        if !claimed(&mime) && !types.contains(&mime) {
            types.push(mime);
        }
    }
    types
}

fn opens(associations: &Associations, app: &str, mime: &MimeType) -> bool {
    associations
        .handlers(mime)
        .iter()
        .any(|entry| entry.id == app)
}

fn icon_theme() -> String {
    Store::open(appearance::schema())
        .ok()
        .and_then(|store| store.get::<String>(ICON_THEME).ok())
        .filter(|theme| !theme.is_empty())
        .unwrap_or_else(|| ICON_THEME_DEFAULT.to_owned())
}

fn no_home() -> String {
    "no home folder to keep choices in".to_owned()
}
