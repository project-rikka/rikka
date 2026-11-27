use std::path::{Path, PathBuf};

use animate_cast::Subject;
use animate_document::Motion;
use animate_look::shared_library;
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_model::mmd::CAMERA_MOTION;

use crate::animate::Animate;

impl Animate {
    pub(crate) fn can_save(&self) -> bool {
        self.subject
            .and_then(|subject| self.motion_of(subject))
            .is_some_and(|motion| !motion.is_empty())
    }

    pub(super) fn saved_motion(&self) -> Option<(String, Motion)> {
        let subject = self.subject?;
        let motion = self
            .motion_of(subject)
            .filter(|motion| !motion.is_empty())?;
        let name = match subject {
            Subject::Figure(id) => self.cast.figure(id)?.name.clone(),
            Subject::Camera => CAMERA_MOTION.to_owned(),
            Subject::Look => tr!("animate.subject.look").to_string(),
            Subject::Emitter(id) => self.cast.emitter(id)?.name.clone(),
            Subject::Wind => tr!("animate.subject.wind").to_string(),
            Subject::Stage | Subject::Accessory(_) => self.cast.prop(subject)?.prop.name.clone(),
        };
        Some((name, motion.document()))
    }

    pub(crate) fn save_motion(&self) {
        let Some((name, motion)) = self.saved_motion() else {
            return;
        };
        let chooser = Chooser::save(tr!("animate.save.title").to_string())
            .parent(window_identifier())
            .accept_label(tr!("animate.save.accept").to_string())
            .current_name(format!("{name}.{}", Motion::EXTENSION))
            .filter(
                Filter::named(tr!("animate.add.rikka_motion_files").to_string())
                    .mime_type(Motion::MIME),
            );
        let chooser = match &self.folder {
            Some(folder) => chooser.current_folder(folder),
            None => chooser,
        };
        spawn_blocking(
            move || match chooser.run() {
                Ok(ChooserResponse::Files(paths)) => match paths.into_iter().next() {
                    Some(path) => {
                        let path = with_extension(path, Motion::EXTENSION);
                        write(&motion, &path).map(Some)
                    }
                    None => Ok(None),
                },
                Ok(ChooserResponse::Cancelled) => Ok(None),
                Err(error) => Err(SaveMotionError::from(error)),
            },
            |animate: &mut Animate, saved: Result<Option<PathBuf>, SaveMotionError>| match saved {
                Ok(Some(path)) => {
                    animate.folder = path.parent().map(PathBuf::from);
                    skip_view();
                }
                Ok(None) => skip_view(),
                Err(error) => animate.notice = Some(Label::from(animate_ui::error_chain(&error))),
            },
        );
    }
}

pub(super) fn with_extension(path: PathBuf, extension: &str) -> PathBuf {
    let has_extension = path
        .extension()
        .is_some_and(|current| current.eq_ignore_ascii_case(extension));
    match has_extension {
        true => path,
        false => {
            let mut path = path.into_os_string();
            path.push(".");
            path.push(extension);
            PathBuf::from(path)
        }
    }
}

fn write(motion: &Motion, path: &Path) -> Result<PathBuf, SaveMotionError> {
    let folder = path.parent().unwrap_or(Path::new(""));
    match motion {
        Motion::Look(look) => {
            let mut portable = (**look).clone();
            portable.map_assets(shared_library(), |asset| {
                match Path::new(asset).strip_prefix(folder) {
                    Ok(inside) => inside.to_string_lossy().into_owned(),
                    Err(_) => asset.to_owned(),
                }
            })?;
            Motion::Look(Box::new(portable)).save(path)?;
        }
        _ => motion.save(path)?,
    }
    Ok(path.to_path_buf())
}

#[derive(Debug, thiserror::Error)]
enum SaveMotionError {
    #[error("cannot choose where to save")]
    Dialog(#[from] rikka_dialog::Error),
    #[error("cannot write the motion file: {0}")]
    Write(#[from] animate_document::Error),
    #[error("cannot make the look's files relative")]
    Look(#[from] animate_look::Error),
}
