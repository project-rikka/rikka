use rikka_text_font::Tag;

use crate::buffer::glyph_buffer::Buffer;
use crate::plan::feature_map::FeatureMap;
use crate::shapers::syllabic::syllables::{next_syllable, syllable_kind};
use crate::shapers::universal::machine::syllable;

pub(crate) const TOPOGRAPHICAL_FEATURES: [&[u8; 4]; 4] = [b"isol", b"init", b"medi", b"fina"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    Isolated,
    Initial,
    Medial,
    Terminal,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Topography {
    masks: [u32; 4],
}

impl Topography {
    pub(crate) fn new(map: &FeatureMap) -> Option<Self> {
        let mut masks = [0; 4];
        for (mask, tag) in masks.iter_mut().zip(TOPOGRAPHICAL_FEATURES) {
            let one = map.one_mask(Tag::new(tag));
            *mask = if one == map.global_mask { 0 } else { one };
        }
        masks
            .iter()
            .any(|&mask| mask != 0)
            .then_some(Self { masks })
    }

    fn apply(&self, buffer: &mut Buffer, start: usize, end: usize, form: Form) {
        let others = !self.masks.iter().fold(0, |all, mask| all | mask);
        let mask = self.masks[form as usize];
        for info in &mut buffer.info[start..end] {
            info.mask = (info.mask & others) | mask;
        }
    }

    pub(crate) fn setup_masks(&self, buffer: &mut Buffer) {
        let mut last_start = 0;
        let mut last_form = None;
        let mut start = 0;
        while start < buffer.len {
            let end = next_syllable(buffer, start);
            let kind = syllable_kind(&buffer.info[start]);
            if kind == syllable::HIEROGLYPH || kind == syllable::NON_CLUSTER {
                last_form = None;
            } else {
                let join = matches!(last_form, Some(Form::Terminal | Form::Isolated));
                if join {
                    let previous = if last_form == Some(Form::Terminal) {
                        Form::Medial
                    } else {
                        Form::Initial
                    };
                    self.apply(buffer, last_start, start, previous);
                }
                let form = if join { Form::Terminal } else { Form::Isolated };
                self.apply(buffer, start, end, form);
                last_form = Some(form);
            }
            last_start = start;
            start = end;
        }
    }
}
