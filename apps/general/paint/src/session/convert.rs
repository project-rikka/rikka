use std::path::Path;

use paint_document::Edit;
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_effects_forms::{form_sheet, labelled};
use rikka_image_edit::{Intent, Profile};
use rikka_widgets::row;

use super::form;
use super::{Context, Dialog, Kind, Outcome, tune};
use crate::paint::Paint;
use crate::wording;

const ICC_MIME: &str = "application/vnd.iccprofile";

const BUILT_IN: [fn() -> Profile; 6] = [
    Profile::srgb,
    Profile::display_p3,
    Profile::adobe_rgb,
    Profile::pro_photo_rgb,
    Profile::rec2020,
    Profile::linear_srgb,
];

#[derive(Debug)]
pub(crate) struct ConvertProfile {
    targets: Vec<Profile>,
    selected: usize,
    intent: Intent,
    error: Option<Label>,
}

impl ConvertProfile {
    pub(crate) fn from_context(_context: &Context<'_>) -> ConvertProfile {
        ConvertProfile {
            targets: BUILT_IN.iter().map(|made| made()).collect(),
            selected: 0,
            intent: Intent::default(),
            error: None,
        }
    }

    fn target(&self) -> Option<&Profile> {
        self.targets.get(self.selected)
    }

    fn loaded(&mut self, loaded: Result<Profile, Label>) {
        match loaded {
            Ok(profile) => {
                self.error = None;
                self.selected = match self
                    .targets
                    .iter()
                    .position(|candidate| *candidate == profile)
                {
                    Some(at) => at,
                    None => {
                        self.targets.push(profile);
                        self.targets.len() - 1
                    }
                };
            }
            Err(error) => self.error = Some(error),
        }
    }
}

impl Dialog for ConvertProfile {
    fn title(&self) -> Label {
        Kind::ConvertProfile.name()
    }

    fn form(&self, context: &Context<'_>) -> Element {
        let names: Vec<Label> = self
            .targets
            .iter()
            .map(|profile| wording::profile(Some(profile)))
            .collect();
        let mut rows = vec![
            labelled(
                tr!("paint.convert.current"),
                text(wording::profile(context.facts.profile.as_ref())).no_wrap(),
            ),
            labelled(
                tr!("paint.convert.target"),
                combo(Some(self.selected))
                    .options(names)
                    .on_select(tune(|convert: &mut ConvertProfile, at: usize| {
                        convert.selected = at;
                    }))
                    .fill_width(),
            ),
            row! {
                button(text(tr!("paint.convert.load")))
                    .small()
                    .on_press(load);
            }
            .any(),
        ];
        if let Some(error) = &self.error {
            rows.push(
                text(error.clone())
                    .caption()
                    .color(palette().danger)
                    .fill_width()
                    .any(),
            );
        }
        rows.push(labelled(
            tr!("paint.convert.intent"),
            form::intent(
                self.intent,
                tune(|convert: &mut ConvertProfile, at: usize| convert.intent = Intent::ALL[at]),
            ),
        ));
        form_sheet(rows)
    }

    fn outcome(&self, _context: &Context<'_>) -> Option<Outcome> {
        Some(Outcome::Edit(Edit::Convert {
            profile: self.target()?.clone(),
            intent: self.intent,
        }))
    }
}

fn load() {
    let chooser = Chooser::open(tr!("paint.convert.load_title").to_string())
        .parent(window_identifier())
        .accept_label(tr!("paint.convert.load_accept").to_string())
        .filter(Filter::named(tr!("paint.convert.profiles").to_string()).mime_type(ICC_MIME));
    spawn_blocking(
        move || match chooser.run() {
            Ok(ChooserResponse::Files(paths)) => paths.first().map(|path| read(path)),
            Ok(ChooserResponse::Cancelled) => None,
            Err(error) => Some(Err(error.to_string())),
        },
        |paint: &mut Paint, loaded: Option<Result<Profile, String>>| match loaded {
            Some(loaded) => paint.tune(|convert: &mut ConvertProfile| {
                convert.loaded(
                    loaded
                        .map_err(|reason| tr!("paint.convert.unloaded", reason = reason.as_str())),
                );
            }),
            None => skip_view(),
        },
    );
}

fn read(path: &Path) -> Result<Profile, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    Profile::from_icc(bytes).map_err(|error| error.to_string())
}
