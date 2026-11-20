use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_model::mmd::Vmd;
use rikka_model::{Error, Import, Model, registry};
use rikka_model_gpu::{Prepared, UploadOptions};
use rikka_types::CancelToken;

#[derive(Debug)]
pub(crate) enum Purpose {
    Motion,
    Figure(Option<Arc<Model>>),
}

#[derive(Debug)]
pub(crate) enum Loaded {
    Motion(Arc<Model>),
    Figure(Prepared),
}

#[derive(Debug)]
pub(crate) struct Load {
    pub(crate) path: PathBuf,
    pub(crate) read: Result<Loaded, Error>,
}

pub(crate) fn read(path: PathBuf, purpose: Purpose, cancel: CancelToken) -> Load {
    let read = open(&path, purpose, cancel);
    Load { path, read }
}

fn open(path: &Path, purpose: Purpose, cancel: CancelToken) -> Result<Loaded, Error> {
    let mut model = Import::open(path, registry()).cancel(cancel).run()?;
    let Purpose::Figure(motion) = purpose else {
        return Ok(Loaded::Motion(Arc::new(model)));
    };
    if let Some(vmd) = motion
        .as_deref()
        .and_then(|motion| motion.extensions.get::<Vmd>())
    {
        model.animations.push(vmd.animation(&model));
    }
    Ok(Loaded::Figure(Prepared::new(
        Arc::new(model),
        &UploadOptions::default(),
    )))
}
