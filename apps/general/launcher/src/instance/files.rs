use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use launcher_instance::Screenshot;
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::format;
use crate::launcher::{Confirm, Launcher};
use crate::load::{Load, loading};
use crate::widgets::{icon_button, lift};

const TILE: f32 = 280.0;
const PICTURE: f32 = 158.0;

#[derive(Default)]
pub(crate) struct Screenshots {
    pub(crate) listed: Load<Rc<[Screenshot]>>,
}

impl Launcher {
    pub(crate) fn load_screenshots(&mut self) {
        let (Some(id), Some(services)) = (self.selected_id(), self.services()) else {
            return;
        };
        if self.instance.screenshots.listed.ready().is_none() {
            self.instance.screenshots.listed = Load::Loading;
        }
        spawn_blocking_keyed(
            "launcher-screenshots",
            move || services.instances.get(&id)?.screenshots(),
            |launcher: &mut Launcher, listed: launcher_instance::Result<Vec<Screenshot>>| {
                launcher.instance.screenshots.listed = match listed {
                    Ok(listed) => Load::Ready(listed.into()),
                    Err(error) => Load::Failed(error.to_string()),
                };
            },
        );
    }

    pub(crate) fn delete_screenshot(&mut self, path: &Path) {
        let path = path.to_owned();
        spawn_blocking(
            move || std::fs::remove_file(&path),
            |launcher: &mut Launcher, removed: std::io::Result<()>| {
                if let Err(error) = removed {
                    launcher.fail(error.to_string());
                }
                launcher.load_screenshots();
            },
        );
    }
}

fn open(launcher: &mut Launcher, path: &Path, reveal: bool) {
    let opened = match reveal {
        true => rikka_open::reveal(path),
        false => rikka_open::open(&path.display().to_string()),
    };
    if let Err(error) = opened {
        launcher.fail(error.to_string());
    }
}

fn tile(shot: &Screenshot) -> Element {
    let opened = shot.path.clone();
    let revealed = shot.path.clone();
    let deleted = shot.path.clone();
    lift(
        container(
            column! {
                container(
                    image(ImageSource::file(Arc::from(shot.path.as_path())))
                        .fit(ImageFit::Cover)
                        .radius(10.0)
                        .fill(),
                )
                .background(palette().surface_variant)
                .radius(10.0)
                .on_press(move |launcher: &mut Launcher| open(launcher, &opened, false))
                .height(Length::px(PICTURE))
                .fill_width();
                row! {
                    column! {
                        text(shot.name.clone()).caption().semibold().no_wrap().fill_width();
                        text(format::ago(shot.taken)).caption().muted().no_wrap();
                    }
                    .gap(Space::HAIR)
                    .grow(1.0)
                    .min_width(0.0);
                    icon_button(MaterialSymbol::FOLDER_OPEN, tr!("launcher.screenshots.reveal"), move |launcher: &mut Launcher| {
                        open(launcher, &revealed, true);
                    });
                    icon_button(MaterialSymbol::DELETE, tr!("launcher.action.delete"), move |launcher: &mut Launcher| {
                        launcher.ask(Confirm::DeleteScreenshot { path: deleted.clone() });
                    });
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center)
                .fill_width();
            }
            .gap(Space::SNUG)
            .fill_width(),
        )
        .padding(Inset::all(Space::SNUG))
        .surface()
        .radius(Corner::Panel)
        .border(1.0, palette().outline)
        .width(Length::px(TILE)),
    )
}

pub(super) fn view(launcher: &Launcher) -> Element {
    loading(
        &launcher.instance.screenshots.listed,
        |shots: &Rc<[Screenshot]>| {
            if shots.is_empty() {
                return status_page(
                    MaterialSymbol::PHOTO_LIBRARY,
                    tr!("launcher.screenshots.empty"),
                )
                .fill()
                .any();
            }
            let shots = Rc::clone(shots);
            let count = shots.len();
            lazy_tiles(Length::px(TILE), count, move |slot| {
                tile(&shots[slot.index])
            })
            .gap(Space::ROOMY)
            .padding(Inset::edges(
                Space::NONE,
                Space::WIDE,
                Space::WIDE,
                Space::WIDE,
            ))
            .fill()
            .any()
        },
    )
}
