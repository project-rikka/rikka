use std::path::PathBuf;

use paint_document::{FileFormat, Saving};
use rikka_app::prelude::*;
use rikka_effects_forms::{form_sheet, labelled, note};
use rikka_image::Compression;

use super::form::{self, Scale};
use super::{Context, Dialog, Outcome, tune};
use crate::wording;

const QUALITY: Scale = Scale::new(1.0..=100.0);
const SPEED: Scale = Scale::new(1.0..=10.0);

const COMPRESSIONS: [Compression; 3] =
    [Compression::Fast, Compression::Balanced, Compression::Best];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SaveChoices {
    pub(crate) quality: u8,
    pub(crate) speed: u8,
    pub(crate) compression: Compression,
}

impl Default for SaveChoices {
    fn default() -> SaveChoices {
        SaveChoices {
            quality: 90,
            speed: 6,
            compression: Compression::Balanced,
        }
    }
}

impl SaveChoices {
    pub(crate) fn saving(self) -> Saving {
        Saving::new()
            .quality(self.quality)
            .speed(self.speed)
            .compression(self.compression)
    }
}

#[derive(Debug)]
pub(crate) struct SaveOptions {
    path: PathBuf,
    format: FileFormat,
    choices: SaveChoices,
}

impl SaveOptions {
    pub(crate) fn asks(format: FileFormat) -> bool {
        format.takes_quality() || format.takes_speed() || format.takes_compression()
    }

    pub(crate) fn new(path: PathBuf, format: FileFormat, choices: SaveChoices) -> SaveOptions {
        SaveOptions {
            path,
            format,
            choices,
        }
    }

    pub(crate) fn remembered(&self) -> (FileFormat, SaveChoices) {
        (self.format, self.choices)
    }
}

impl Dialog for SaveOptions {
    fn title(&self) -> Label {
        tr!("paint.save_options.title", format = self.format.name())
    }

    fn confirm(&self) -> Label {
        tr!("paint.save_options.save")
    }

    fn form(&self, _context: &Context<'_>) -> Element {
        let mut rows = vec![note(Label::from(wording::file_name(&self.path)))];
        if self.format.takes_quality() {
            rows.push(labelled(
                tr!("paint.save_options.quality"),
                form::sliding(
                    f32::from(self.choices.quality),
                    &QUALITY,
                    tune(|options: &mut SaveOptions, quality: f32| {
                        options.choices.quality = quality.round().clamp(1.0, 100.0) as u8;
                    }),
                ),
            ));
        }
        if self.format.takes_speed() {
            rows.push(labelled(
                tr!("paint.save_options.speed"),
                form::sliding(
                    f32::from(self.choices.speed),
                    &SPEED,
                    tune(|options: &mut SaveOptions, speed: f32| {
                        options.choices.speed = speed.round().clamp(1.0, 10.0) as u8;
                    }),
                ),
            ));
            rows.push(note(tr!("paint.save_options.speed_note")));
        }
        if self.format.takes_compression() {
            let at = COMPRESSIONS
                .iter()
                .position(|compression| *compression == self.choices.compression);
            rows.push(labelled(
                tr!("paint.save_options.compression"),
                combo(at)
                    .options([
                        tr!("paint.save_options.fast"),
                        tr!("paint.save_options.balanced"),
                        tr!("paint.save_options.best"),
                    ])
                    .on_select(tune(|options: &mut SaveOptions, at: usize| {
                        options.choices.compression = COMPRESSIONS[at];
                    }))
                    .fill_width(),
            ));
        }
        if !self.format.keeps_alpha() {
            rows.push(note(tr!("paint.save_options.flattened")));
        }
        form_sheet(rows)
    }

    fn outcome(&self, _context: &Context<'_>) -> Option<Outcome> {
        Some(Outcome::Save {
            path: self.path.clone(),
            saving: self.choices.saving(),
        })
    }
}
