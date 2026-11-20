mod details;
mod draft;
mod progress;
mod sheet;
mod wording;

use rikka_app::prelude::*;

use crate::cut::Cut;

pub(crate) use draft::{Exporting, Opening};

pub(crate) fn view(cut: &Cut) -> Vec<Element> {
    if let Some(job) = cut.job() {
        return progress::sheet(cut, job);
    }
    match cut.exporting() {
        Some(exporting) => sheet::sheet(cut, exporting),
        None => Vec::new(),
    }
}
