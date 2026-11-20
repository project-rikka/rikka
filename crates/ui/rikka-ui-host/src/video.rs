use std::time::Duration;

use rikka_picture::VideoController;
use rikka_renderer::{TextureId, Uploads};
use rikka_ui::SourceKey;

pub trait VideoPlayback: std::fmt::Debug {
    fn advance(&mut self, uploads: &Uploads) -> VideoFrame;

    fn resolve(&mut self, controller: &VideoController) -> Option<TextureId>;

    fn is_starting(&self) -> bool;

    fn has_work(&self) -> bool;
}

#[derive(Clone, Debug, Default)]
pub struct VideoFrame {
    pub updated: Vec<TextureId>,
    pub settled: Vec<SourceKey>,
    pub due: Option<Duration>,
}
