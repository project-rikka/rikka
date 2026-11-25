mod content;
mod files;
mod logs;
mod options;
mod worlds;

use std::sync::Arc;

use launcher_install::Stage;
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

pub(crate) use content::ContentState;
pub(crate) use files::Screenshots;
pub(crate) use logs::LogState;
pub(crate) use options::OptionsForm;
pub(crate) use worlds::WorldsState;

use crate::format;
use crate::launcher::Launcher;
use crate::pictures;
use crate::runs::Activity;
use crate::target;
use crate::widgets::{chip, icon_button, labelled, stat};

const ICON: f32 = 72.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Tab {
    #[default]
    Content,
    Worlds,
    Screenshots,
    Logs,
    Options,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Content,
        Tab::Worlds,
        Tab::Screenshots,
        Tab::Logs,
        Tab::Options,
    ];

    fn label(self) -> Label {
        match self {
            Tab::Content => tr!("launcher.instance.tab.content"),
            Tab::Worlds => tr!("launcher.instance.tab.worlds"),
            Tab::Screenshots => tr!("launcher.instance.tab.screenshots"),
            Tab::Logs => tr!("launcher.instance.tab.logs"),
            Tab::Options => tr!("launcher.instance.tab.options"),
        }
    }
}

#[derive(Default)]
pub(crate) struct InstancePage {
    pub(crate) tab: Tab,
    pub(crate) content: ContentState,
    pub(crate) worlds: WorldsState,
    pub(crate) screenshots: Screenshots,
    pub(crate) logs: LogState,
    pub(crate) options: Option<OptionsForm>,
}

impl std::fmt::Debug for InstancePage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstancePage")
            .field("tab", &self.tab)
            .finish_non_exhaustive()
    }
}

impl Launcher {
    pub(crate) fn selected_id(&self) -> Option<String> {
        self.selected.clone()
    }

    fn set_tab(&mut self, tab: Tab) {
        self.instance.tab = tab;
        self.load_instance_tab();
    }

    pub(crate) fn load_instance_tab(&mut self) {
        match self.instance.tab {
            Tab::Content => self.load_content(),
            Tab::Worlds => self.load_worlds(),
            Tab::Screenshots => self.load_screenshots(),
            Tab::Logs => self.load_log_files(),
            Tab::Options => self.open_options(),
        }
    }

    fn open_folder(&mut self, id: String) {
        let Some(instance) = self.instance(&id) else {
            return;
        };
        if let Err(error) = rikka_open::open(&instance.path.display().to_string()) {
            self.fail(error.to_string());
        }
    }
}

fn stage_label(stage: Stage) -> Label {
    match stage {
        Stage::Metadata => tr!("launcher.run.stage.metadata"),
        Stage::Java => tr!("launcher.run.stage.java"),
        Stage::Libraries => tr!("launcher.run.stage.libraries"),
        Stage::Assets => tr!("launcher.run.stage.assets"),
    }
}

fn play_area(launcher: &Launcher, id: &str) -> Element {
    let played = id.to_owned();
    let stopped = id.to_owned();
    match launcher.runs.activity(id) {
        Activity::Idle => button(labelled(
            MaterialSymbol::PLAY_ARROW,
            tr!("launcher.action.play"),
        ))
        .primary()
        .large()
        .on_press(move |launcher: &mut Launcher| launcher.play(played.clone(), None))
        .any(),
        Activity::Running => button(labelled(MaterialSymbol::STOP, tr!("launcher.action.stop")))
            .danger()
            .large()
            .on_press(move |launcher: &mut Launcher| launcher.stop(&stopped))
            .any(),
        Activity::Preparing(fraction) => {
            let detail = launcher.runs.progress(id).map(|progress| {
                let mut line = stage_label(progress.stage).to_string();
                if progress.total_files > 0 {
                    line.push_str(&format!(" · {}/{}", progress.files, progress.total_files));
                }
                if progress.total_bytes > 0 {
                    line.push_str(&format!(
                        " · {} / {}",
                        format::bytes(progress.bytes),
                        format::bytes(progress.total_bytes)
                    ));
                }
                line
            });
            row! {
                column! {
                    match fraction {
                        Some(fraction) => progress(fraction).fill_width().any(),
                        None => progress_indeterminate().fill_width().any(),
                    };
                    text(detail.unwrap_or_else(|| tr!("launcher.run.preparing").to_string()))
                        .caption()
                        .muted()
                        .no_wrap();
                }
                .gap(Space::TIGHT)
                .width(Length::px(260.0));
                button(text(tr!("launcher.action.cancel")))
                    .ghost()
                    .on_press(move |launcher: &mut Launcher| launcher.stop(&stopped));
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center)
            .any()
        }
    }
}

fn header(launcher: &Launcher, id: &str) -> Option<Element> {
    let instance = launcher.instance(id)?;
    let target = target::rikka();
    let game = instance.config.game.clone();
    let outdated = game.as_ref().is_some_and(|game| {
        game.minecraft != target.minecraft || game.fabric_loader != target.fabric_loader
    });
    let minecraft = game
        .as_ref()
        .map_or(target.minecraft.clone(), |game| game.minecraft.clone());
    let loader = game.as_ref().map_or(target.fabric_loader.clone(), |game| {
        game.fabric_loader.clone()
    });
    let folder = id.to_owned();
    Some(
        row! {
            pictures::framed(
                instance.icon().map(|path| ImageSource::file(Arc::from(path))),
                MaterialSymbol::SPORTS_ESPORTS,
                ICON,
                18.0,
            );
            column! {
                text(instance.config.name.clone()).title().bold().no_wrap().fill_width();
                row! {
                    chip(tr!("launcher.instance.minecraft", version = minecraft.as_str()), palette().accent);
                    chip(tr!("launcher.instance.fabric", version = loader.as_str()), palette().text_muted);
                    if outdated {
                        chip(
                            tr!("launcher.instance.update-pending", version = target.minecraft.as_str()),
                            palette().warning,
                        );
                    }
                }
                .gap(Space::TIGHT);
                row! {
                    stat(MaterialSymbol::SCHEDULE, format::play_time(instance.config.play_time));
                    stat(
                        MaterialSymbol::HISTORY,
                        instance
                            .config
                            .last_played
                            .map_or_else(|| tr!("launcher.time.never-played"), format::ago),
                    );
                }
                .gap(Space::NORMAL);
            }
            .gap(Space::SNUG)
            .grow(1.0)
            .min_width(0.0);
            icon_button(
                MaterialSymbol::FOLDER_OPEN,
                tr!("launcher.instance.open-folder"),
                move |launcher: &mut Launcher| launcher.open_folder(folder.clone()),
            );
            play_area(launcher, id);
        }
        .gap(Space::ROOMY)
        .align_items(Align::Center)
        .padding(Inset::edges(Space::LOOSE, Space::WIDE, Space::ROOMY, Space::WIDE))
        .fill_width()
        .any(),
    )
}

pub(crate) fn view(launcher: &Launcher) -> Element {
    let Some(id) = launcher.selected_id() else {
        return status_page(
            MaterialSymbol::SPORTS_ESPORTS,
            tr!("launcher.instance.none"),
        )
        .fill()
        .any();
    };
    let Some(header) = header(launcher, &id) else {
        return status_page(
            MaterialSymbol::SPORTS_ESPORTS,
            tr!("launcher.instance.none"),
        )
        .fill()
        .any();
    };
    let tab = launcher.instance.tab;
    let body = match tab {
        Tab::Content => content::view(launcher, &id),
        Tab::Worlds => worlds::view(launcher, &id),
        Tab::Screenshots => files::view(launcher),
        Tab::Logs => logs::view(launcher, &id),
        Tab::Options => options::view(launcher, &id),
    };
    column! {
        header;
        container(
            segmented(Tab::ALL.iter().position(|shown| *shown == tab).unwrap_or_default())
                .segments(Tab::ALL.map(Tab::label))
                .on_select(|launcher: &mut Launcher, index: usize| {
                    if let Some(tab) = Tab::ALL.get(index) {
                        launcher.set_tab(*tab);
                    }
                }),
        )
        .padding(Inset::xy(Space::WIDE, Space::NONE));
        page_stack(tab as usize, body).grow(1.0).fill_width();
    }
    .gap(Space::ROOMY)
    .fill()
    .any()
}
