use rikka_figure::{Lettering, TextAlign};

use super::Paint;
use crate::figures::Restyle;
use crate::options::{BOX_WIDEST, Letters};

impl Paint {
    pub(crate) fn letters(&self) -> &Letters {
        &self.options.letters
    }

    pub(crate) fn commit_text(&mut self) {
        let letters = self.options.letters.clone();
        let color = self.colors.primary;
        let refusal = self
            .active_tab_mut()
            .and_then(|tab| tab.commit_text(&letters, color));
        if refusal.is_some() {
            self.notice = refusal;
        }
    }

    pub(crate) fn set_font(&mut self, font: String) {
        self.options.letters.font = font;
        self.restyle_letters();
    }

    pub(crate) fn set_text_size(&mut self, size: f32) {
        self.options.letters.size = size.clamp(Lettering::SMALLEST, Lettering::LARGEST);
        self.restyle_letters();
    }

    pub(crate) fn toggle_bold(&mut self) {
        self.options.letters.bold = !self.options.letters.bold;
        self.restyle_letters();
    }

    pub(crate) fn toggle_italic(&mut self) {
        self.options.letters.italic = !self.options.letters.italic;
        self.restyle_letters();
    }

    pub(crate) fn set_text_align(&mut self, align: TextAlign) {
        self.options.letters.align = align;
        self.restyle_letters();
    }

    pub(crate) fn set_antialias(&mut self, antialias: bool) {
        self.options.letters.antialias = antialias;
        self.restyle_letters();
    }

    pub(crate) fn set_wraps(&mut self, wraps: bool) {
        self.options.letters.wraps = wraps;
        self.restyle_letters();
    }

    pub(crate) fn set_box_width(&mut self, width: f32) {
        self.options.letters.box_width = width.round().clamp(1.0, BOX_WIDEST) as u32;
        self.restyle_letters();
    }

    fn restyle_letters(&mut self) {
        self.restyle(Restyle::Letters(self.options.letters.clone()), true);
    }
}
