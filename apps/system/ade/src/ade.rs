use std::path::PathBuf;

use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_llm_accounts::AccountsChanged;
use rikka_widgets::row;

use crate::project::{self, Project, ProjectHandle};
use crate::screen::Screen;
use crate::settings::{self, Section, Settings};
use crate::setup::Setup;
use crate::{labels, welcome};

#[derive(State)]
pub(crate) struct Ade {
    setup: Setup,
    screen: Screen,
    #[state]
    project: Option<Project>,
    #[state]
    settings: Settings,
    handle: ProjectHandle,
}

impl Ade {
    pub(crate) fn new(sender: Sender<Ade>) -> Ade {
        Ade {
            settings: Settings::default(),
            setup: Setup::load(),
            screen: Screen::default(),
            project: None,
            handle: ProjectHandle::new(sender),
        }
    }

    pub(crate) fn start(&mut self) {
        if let Some(changes) = self.setup.llm.changes() {
            stream(changes, |ade: &mut Ade, announced: AccountsChanged| {
                ade.settings_changed(announced)
            });
        }
    }

    pub(crate) fn setup(&self) -> &Setup {
        &self.setup
    }

    pub(crate) fn project(&self) -> Option<&Project> {
        self.project.as_ref()
    }

    pub(crate) fn work<R>(&mut self, act: impl FnOnce(&mut Project, &mut Setup) -> R) -> Option<R> {
        let project = self.project.as_mut()?;
        let done = act(project, &mut self.setup);
        project.settle();
        Some(done)
    }

    pub(crate) fn open(&mut self, project: PathBuf) {
        if !project.is_dir() {
            self.setup.config.remove_recent(&project);
            return;
        }
        self.setup.config.add_recent(&project);
        self.project = Some(Project::open(project, &self.setup, &self.handle));
        self.screen = Screen::Work;
    }

    pub(crate) fn open_settings(&mut self, section: Section) {
        self.settings.select(section);
        self.show(Screen::Settings);
    }

    fn show(&mut self, screen: Screen) {
        self.screen = screen;
        if screen != Screen::Settings {
            self.settings.leave();
            return;
        }

        let (root, hub) = match &self.project {
            Some(project) => (Some(project.root().to_owned()), project.hub().clone()),
            None => (None, project::hub(None)),
        };
        self.settings.reread(root, hub);
    }

    fn settings_changed(&mut self, announced: AccountsChanged) {
        self.setup.llm.apply_change(announced);
        self.work(|project, setup| project.reconsider(setup));
    }
}

pub(crate) fn ask() {
    let title = tr!("ade.welcome.chooser").to_string();
    let accept = tr!("ade.welcome.accept").to_string();
    let parent = window_identifier();
    spawn_blocking(
        move || chosen(&title, &accept, parent),
        |ade: &mut Ade, picked: Option<PathBuf>| match picked {
            Some(project) => ade.open(project),
            None => skip_view(),
        },
    );
}

pub(crate) fn view(ade: &Ade) -> impl View {
    let screen = match ade.screen {
        Screen::Work => work(ade),
        Screen::Settings => settings::view(&ade.settings).any(),
    };
    page_stack(ade.screen.at(ade.project.is_some()), screen).fill()
}

fn work(ade: &Ade) -> Element {
    match &ade.project {
        Some(_) => project::view().any(),
        None => welcome::view(&ade.setup).any(),
    }
}

pub(crate) fn bar(ade: &Ade) -> Option<CaptionBar> {
    let bar = match (ade.screen, &ade.project) {
        (Screen::Settings, _) => caption()
            .leading(
                button(
                    row! {
                        icon(MaterialSymbol::ARROW_BACK).extent(Length::px(16.0));
                        text(tr!("ade.settings.back")).label();
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .ghost()
                .small()
                .on_press(|ade: &mut Ade| ade.show(Screen::Work)),
            )
            .center(text(tr!("ade.settings.title")).label().medium().no_wrap()),
        (Screen::Work, Some(project)) => caption()
            .leading(
                row! {
                    icon(MaterialSymbol::FOLDER_OPEN).extent(Length::px(16.0));
                    text(labels::file_name(project.root())).label().medium().no_wrap();
                    project::place();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .center(project::heading())
            .trailing(settings()),
        (Screen::Work, None) => caption()
            .leading(
                row! {
                    icon(MaterialSymbol::AUTO_AWESOME).extent(Length::px(16.0));
                    text(tr!("ade.title")).label().medium().no_wrap();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .trailing(settings()),
    };
    Some(bar.into())
}

fn settings() -> impl View {
    button(icon(MaterialSymbol::SETTINGS).extent(Length::px(17.0)))
        .icon_only()
        .ghost()
        .small()
        .on_press(|ade: &mut Ade| ade.show(Screen::Settings))
}

fn chosen(title: &str, accept: &str, parent: Option<WindowIdentifier>) -> Option<PathBuf> {
    match Chooser::open(title)
        .parent(parent)
        .accept_label(accept)
        .directory(true)
        .run()
    {
        Ok(ChooserResponse::Files(picked)) => picked.into_iter().next(),
        Ok(ChooserResponse::Cancelled) => None,
        Err(error) => {
            tracing::warn!(%error, "failed to open folder picker");
            None
        }
    }
}

impl std::fmt::Debug for Ade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ade")
            .field("screen", &self.screen)
            .field("project", &self.project)
            .finish_non_exhaustive()
    }
}
