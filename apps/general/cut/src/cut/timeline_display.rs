use cut_filmstrip::Mode;
use cut_project::{Edit, Height};
use cut_waveform::Scale;

use super::Cut;
use crate::media_previews::{Display, MediaPreviews};

impl Cut {
    pub(crate) fn media_previews(&self) -> &MediaPreviews {
        &self.media_previews
    }

    pub(crate) fn display(&self) -> Display {
        self.display
    }

    pub(crate) fn show_thumbnails(&mut self, thumbnails: Mode) {
        self.redisplay(Display {
            thumbnails,
            ..self.display
        });
    }

    pub(crate) fn show_waveforms(&mut self, waveform: Scale) {
        self.redisplay(Display {
            waveform,
            ..self.display
        });
    }

    fn redisplay(&mut self, display: Display) {
        self.display = display;
        self.preferences.keep_display(display);
    }

    pub(crate) fn resizing(&self) -> Option<(usize, Height)> {
        self.session.resizing
    }

    pub(crate) fn pull_layer(&mut self, layer: usize, height: Height) {
        self.session.resizing = Some((layer, height));
    }

    pub(crate) fn resize_layer(&mut self, layer: usize, height: Height) {
        self.session.resizing = None;
        self.edit(Edit::Resize { layer, height });
    }

    pub(crate) fn resize_layers(&mut self, height: Height) {
        self.edit(Edit::ResizeAll(height));
    }
}
