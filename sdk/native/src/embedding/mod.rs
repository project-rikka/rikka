mod device;
mod surface;

use std::rc::Rc;

use rikka_app::embedded::{self, Background};
use rikka_i18n::I18n;
use rikka_ui_video::VideoStore;

use crate::abi::violation;

pub(crate) use device::{Handles, Submit, VulkanDevice};
pub(crate) use surface::{EmbeddedSurface, EmbeddedSurfaceBuilder};

#[derive(Debug)]
pub(crate) struct Embedding {
    drawing: Result<Rc<embedded::Embedding>, String>,
}

impl Embedding {
    pub(crate) fn new(device: VulkanDevice) -> Self {
        let drawing = embedded::Embedding::new(device.into_device(), I18n::builder())
            .map(|drawing| Rc::new(drawing.plays_video(VideoStore::default())))
            .map_err(|failure| failure.to_string());
        Self { drawing }
    }

    pub(crate) fn error_message(&self) -> String {
        match &self.drawing {
            Ok(_) => String::new(),
            Err(failure) => failure.clone(),
        }
    }

    pub(crate) fn open(&self, surface: EmbeddedSurfaceBuilder) -> EmbeddedSurface {
        surface.open(self.drawing())
    }

    pub(crate) fn set_theme(&self, slug: &str) {
        self.drawing()
            .set_theme((!slug.is_empty()).then(|| slug.to_owned()));
    }

    pub(crate) fn set_background(&self, background: Option<Background>) {
        self.drawing().set_background(background);
    }

    fn drawing(&self) -> &Rc<embedded::Embedding> {
        match &self.drawing {
            Ok(embedding) => embedding,
            Err(failure) => violation(format!("the embedding cannot draw: {failure}")),
        }
    }
}
