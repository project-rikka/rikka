use std::sync::Arc;
use std::sync::mpsc::Sender;

use rikka_model::{Import, Model, registry};
use rikka_model_gpu::{Prepared, UploadOptions};
use rikka_renderer::Uploads;
use rikka_types::PixelSize;

use crate::delivery::Delivery;
use crate::error::reason;
use crate::loader::Loader;
use crate::setup::Setup;
use crate::sitter::{ModelKey, Sitter, Source};

#[derive(Debug)]
pub(crate) struct Job {
    pub(crate) sitter: Sitter,
    pub(crate) size: PixelSize,
    pub(crate) delivery: Delivery,
}

#[derive(Debug)]
pub(crate) enum Task {
    Load(Job),
    Prepare {
        key: ModelKey,
        source: Source,
        uploads: Uploads,
    },
}

#[derive(Debug)]
pub(crate) struct Staged {
    pub(crate) setup: Arc<Setup>,
    pub(crate) size: PixelSize,
    pub(crate) delivery: Delivery,
}

pub(crate) enum Message {
    Staged(Box<Staged>),
    Seat {
        key: ModelKey,
        prepared: Box<Prepared>,
    },
    Unseated {
        key: ModelKey,
        reason: Arc<str>,
    },
    Roused,
    Stop,
}

fn model_of(source: Source) -> Result<Arc<Model>, rikka_model::Error> {
    match source {
        Source::Model(model) => Ok(model),
        Source::File(stamp) => Ok(Arc::new(Import::open(stamp.path(), registry()).run()?)),
    }
}

pub(crate) fn run(loader: &Loader, studio: &Sender<Message>, task: Task) {
    let message = match task {
        Task::Load(job) => {
            if job.delivery.is_cancelled() {
                return;
            }
            match loader.setup(&job.sitter) {
                Ok(setup) => Message::Staged(Box::new(Staged {
                    setup,
                    size: job.size,
                    delivery: job.delivery,
                })),
                Err(error) => return job.delivery.fail(error),
            }
        }
        Task::Prepare {
            key,
            source,
            uploads,
        } => match model_of(source) {
            Ok(model) => Message::Seat {
                key,
                prepared: Box::new(Prepared::with_uploads(
                    model,
                    &UploadOptions::default(),
                    Some(&uploads),
                )),
            },
            Err(error) => Message::Unseated {
                key,
                reason: reason(&error),
            },
        },
    };
    if studio.send(message).is_err() {
        tracing::debug!("booth stopped before a load finished");
    }
}
