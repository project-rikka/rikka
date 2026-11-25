use launcher_content::Plan;
use launcher_instance::{ContentKind, Instance, Place, World, Worlds};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::instance::Tab;
use crate::launcher::{Launcher, Page, STAGE};
use crate::load::Load;
use crate::services::{scan, update};
use crate::target;

const SHEET: f32 = 420.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorldPick {
    project: String,
    version: Option<String>,
    worlds: Load<Vec<World>>,
}

type Installed = Result<(Instance, Plan), String>;

impl Launcher {
    pub(crate) fn install(&mut self, project: String, kind: ContentKind, version: Option<String>) {
        if self.discover.target.is_none() {
            self.fail(tr!("launcher.discover.no-target"));
            return;
        }
        match kind {
            ContentKind::DataPack => self.pick_world(project, version),
            ContentKind::Mod | ContentKind::ResourcePack | ContentKind::ShaderPack => {
                self.install_into(project, version, Place::Instance)
            }
        }
    }

    fn pick_world(&mut self, project: String, version: Option<String>) {
        let (Some(id), Some(services)) = (self.discover.target.clone(), self.services()) else {
            return;
        };
        self.world_pick = Some(WorldPick {
            project,
            version,
            worlds: Load::Loading,
        });
        spawn_blocking(
            move || services.instances.get(&id)?.worlds(),
            |launcher: &mut Launcher, worlds: launcher_instance::Result<Worlds>| {
                let Some(pick) = &mut launcher.world_pick else {
                    return;
                };
                match worlds {
                    Ok(listed) => {
                        pick.worlds = Load::Ready(listed.worlds);
                        if let Some(problem) = listed.problems.first() {
                            launcher.fail(problem.to_string());
                        }
                    }
                    Err(error) => pick.worlds = Load::Failed(error.to_string()),
                }
            },
        );
    }

    fn cancel_world_pick(&mut self) {
        self.world_pick = None;
    }

    fn choose_world(&mut self, folder: String) {
        let Some(pick) = self.world_pick.take() else {
            return;
        };
        self.install_into(pick.project, pick.version, Place::World(folder));
    }

    fn install_into(&mut self, project: String, version: Option<String>, place: Place) {
        let (Some(id), Some(services)) = (self.discover.target.clone(), self.services()) else {
            return;
        };
        self.discover.installing.insert(project.clone());
        let game = target::rikka().minecraft;
        let requested = project.clone();
        spawn(
            async move {
                let (mut instance, files) = scan(services.clone(), id.clone())
                    .await
                    .map_err(|error| error.to_string())?;
                let plan = services
                    .catalog
                    .plan(&files, &requested, place, &game, version.as_deref())
                    .await
                    .map_err(|error| error.to_string())?;
                services
                    .catalog
                    .apply(&mut instance, &plan)
                    .await
                    .map_err(|error| error.to_string())?;
                let content = instance.config.content;
                let instance = update(services, id, move |config| config.content = content)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok((instance, plan))
            },
            move |launcher: &mut Launcher, installed: Installed| {
                launcher.discover.installing.remove(&project);
                match installed {
                    Ok((instance, plan)) => {
                        let shown = instance.id.clone();
                        launcher.replace_instance(instance);
                        let title = plan
                            .items
                            .first()
                            .map(|item| item.project.title.clone())
                            .unwrap_or_default();
                        match plan.items.len() {
                            0 => launcher.tell(tr!("launcher.discover.already")),
                            1 => {
                                launcher.tell(tr!("launcher.discover.done", name = title.as_str()))
                            }
                            count => launcher.tell(tr!(
                                "launcher.discover.done-with",
                                name = title.as_str(),
                                count = &(count - 1).to_string()
                            )),
                        }
                        if launcher.page == Page::Instance
                            && launcher.selected.as_deref() == Some(shown.as_str())
                            && launcher.instance.tab == Tab::Content
                        {
                            launcher.load_content();
                        }
                    }
                    Err(error) => {
                        launcher.fail(tr!("launcher.discover.failed", reason = error.as_str()))
                    }
                }
            },
        );
    }
}

pub(crate) fn world_sheet(launcher: &Launcher) -> Vec<Element> {
    let Some(pick) = &launcher.world_pick else {
        return Vec::new();
    };
    let body = match &pick.worlds {
        Load::Idle | Load::Loading => container(spinner().extent(Length::px(22.0)))
            .center()
            .padding(Inset::all(Space::ROOMY))
            .fill_width()
            .any(),
        Load::Failed(error) => text(error.clone()).label().danger().any(),
        Load::Ready(worlds) if worlds.is_empty() => text(tr!("launcher.discover.no-worlds"))
            .label()
            .muted()
            .fill_width()
            .any(),
        Load::Ready(worlds) => column_of(worlds.iter().map(|world| {
            let folder = world.folder.clone();
            button(
                row! {
                    icon(MaterialSymbol::PUBLIC).extent(Length::px(18.0));
                    text(world.name.clone()).label().no_wrap();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .ghost()
            .on_press(move |launcher: &mut Launcher| launcher.choose_world(folder.clone()))
            .fill_width()
            .any()
        }))
        .gap(Space::HAIR)
        .fill_width()
        .any(),
    };
    sheet(
        column! {
            text(tr!("launcher.discover.choose-world")).label().semibold();
            text(tr!("launcher.discover.choose-world-body")).caption().muted().fill_width();
            body;
            row! {
                spacer().grow(1.0);
                button(text(tr!("launcher.action.cancel")))
                    .ghost()
                    .on_press(Launcher::cancel_world_pick);
            }
            .fill_width();
        }
        .gap(Space::ROOMY)
        .padding(20.0)
        .fill_width(),
    )
    .width(SHEET)
    .on_dismiss(Launcher::cancel_world_pick)
    .keyed("launcher-world-pick")
    .centered_on(STAGE)
    .with_scrim()
}
