use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_dirs::{UserDir, UserDirs};
use rikka_widgets::{column, row};
use settings_avatar::{Framing, MAX_ZOOM, Photo, Preview};
use tempfile::TempPath;

use crate::users::component::{Report, Users};
use crate::users::cropper::Cropper;

const PREVIEW: u32 = 2048;
const SIDE: f32 = 320.0;
const ZOOM_STEP: f32 = 0.01;
const STAGED_PREFIX: &str = "rikka-avatar-";
const STAGED_SUFFIX: &str = ".png";

pub(crate) struct Cropping {
    photo: Arc<Photo>,
    preview: ImageSource,
    framing: Framing,
    saving: bool,
}

pub(crate) struct Opened {
    photo: Photo,
    preview: Preview,
}

impl Cropping {
    fn new(opened: Opened) -> Cropping {
        Cropping {
            framing: opened.photo.framing(),
            photo: Arc::new(opened.photo),
            preview: ImageSource::Rgba {
                width: opened.preview.width,
                height: opened.preview.height,
                pixels: Arc::from(opened.preview.rgba),
            },
            saving: false,
        }
    }
}

impl Users {
    pub(crate) fn choose_picture(&mut self) {
        let pictures = Photo::mime_types().into_iter().fold(
            Filter::named(tr!("settings.users.pictures").to_string()),
            Filter::mime_type,
        );
        let chooser = Chooser::open(tr!("settings.users.choose-title").to_string())
            .parent(window_identifier())
            .accept_label(tr!("settings.users.choose-accept").to_string())
            .filter(pictures);
        self.busy = true;
        spawn_blocking(
            move || {
                let chooser = match UserDirs::load().path(UserDir::Pictures) {
                    Some(folder) => chooser.current_folder(folder),
                    None => chooser,
                };
                match chooser.run() {
                    Ok(ChooserResponse::Files(files)) => files.first().map(|file| open(file)),
                    Ok(ChooserResponse::Cancelled) => None,
                    Err(error) => Some(Err(error.to_string())),
                }
            },
            |users: &mut Users, opened: Option<Result<Opened, String>>| {
                users.busy = false;
                match opened {
                    None => {}
                    Some(Ok(opened)) => {
                        users.report = None;
                        users.cropping = Some(Cropping::new(opened));
                    }
                    Some(Err(reason)) => users.report = Some(Report::PictureUnreadable(reason)),
                }
            },
        );
    }

    pub(crate) fn remove_picture(&mut self) {
        let Some(accounts) = self.accounts().cloned() else {
            return;
        };
        self.busy = true;
        spawn(
            async move { accounts.clear_icon().await },
            |users: &mut Users, cleared: rikka_accounts::Result<()>| {
                users.busy = false;
                match cleared {
                    Ok(()) => users.read(),
                    Err(error) => users.report = Some(Report::PictureRefused(error.to_string())),
                }
            },
        );
    }

    fn reframe(&mut self, framing: Framing) {
        if let Some(cropping) = &mut self.cropping {
            cropping.framing = framing;
        }
    }

    fn zoom_to(&mut self, zoom: f32) {
        if let Some(cropping) = &mut self.cropping {
            cropping.framing = cropping.framing.zoomed(zoom, cropping.framing.center());
        }
    }

    fn stop_cropping(&mut self) {
        self.cropping = None;
    }

    fn save_picture(&mut self) {
        let Some(cropping) = &mut self.cropping else {
            return;
        };
        cropping.saving = true;
        let photo = Arc::clone(&cropping.photo);
        let framing = cropping.framing;
        spawn_blocking(
            move || stage(&photo, &framing),
            |users: &mut Users, staged: Result<TempPath, String>| match staged {
                Ok(file) => users.apply_picture(file),
                Err(reason) => users.stop_saving(Report::PictureRefused(reason)),
            },
        );
    }

    fn apply_picture(&mut self, file: TempPath) {
        let Some(accounts) = self.accounts().cloned() else {
            self.stop_saving(Report::AccountsLost);
            return;
        };
        spawn(
            async move { accounts.set_icon(&file).await },
            |users: &mut Users, set: rikka_accounts::Result<()>| match set {
                Ok(()) => {
                    users.cropping = None;
                    users.read();
                }
                Err(error) => users.stop_saving(Report::PictureRefused(error.to_string())),
            },
        );
    }

    fn stop_saving(&mut self, report: Report) {
        if let Some(cropping) = &mut self.cropping {
            cropping.saving = false;
        }
        self.report = Some(report);
    }
}

fn open(file: &Path) -> Result<Opened, String> {
    let failed = |error: settings_avatar::Error| format!("{}: {error}", file.display());
    let photo = Photo::open(file).map_err(failed)?;
    let preview = photo.preview(PREVIEW).map_err(failed)?;
    Ok(Opened { photo, preview })
}

fn stage(photo: &Photo, framing: &Framing) -> Result<TempPath, String> {
    let png = photo.avatar(framing).map_err(|error| error.to_string())?;
    let mut file = tempfile::Builder::new()
        .prefix(STAGED_PREFIX)
        .suffix(STAGED_SUFFIX)
        .tempfile()
        .map_err(|error| error.to_string())?;
    file.write_all(&png).map_err(|error| error.to_string())?;
    file.flush().map_err(|error| error.to_string())?;
    Ok(file.into_temp_path())
}

pub(crate) fn view(cropping: &Cropping) -> impl View {
    let saving = cropping.saving;
    section(
        tr!("settings.users.crop-title"),
        column! {
            Element::new(Cropper::new(
                cropping.preview.clone(),
                cropping.framing,
                |users: &mut Users, framing: Framing| users.reframe(framing),
            ))
            .width(Length::px(SIDE))
            .height(Length::px(SIDE))
            .align_self(Align::Center);
            row! {
                icon(MaterialSymbol::ZOOM_OUT).color(palette().text_muted);
                slider(cropping.framing.zoom())
                    .range(1.0..=MAX_ZOOM)
                    .step(ZOOM_STEP)
                    .enabled(!saving)
                    .on_change(|users: &mut Users, zoom: f32| users.zoom_to(zoom))
                    .grow(1.0);
                icon(MaterialSymbol::ZOOM_IN).color(palette().text_muted);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .width(Length::px(SIDE))
            .align_self(Align::Center);
            row! {
                spacer().grow(1.0);
                button(tr!("settings.users.cancel"))
                    .secondary()
                    .enabled(!saving)
                    .on_press(Users::stop_cropping);
                button(tr!("settings.users.crop-apply"))
                    .primary()
                    .loading(saving)
                    .enabled(!saving)
                    .on_press(Users::save_picture);
            }
            .gap(Space::SNUG)
            .fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
}

impl std::fmt::Debug for Cropping {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cropping")
            .field("framing", &self.framing)
            .field("saving", &self.saving)
            .finish_non_exhaustive()
    }
}
