use rikka_text_font::{LangSys, LayoutTable, Tag};

use crate::plan::ot_tags::OpenTypeTags;

const FALLBACK_SCRIPTS: [Tag; 3] = [Tag::new(b"DFLT"), Tag::new(b"dflt"), Tag::new(b"latn")];
const DEFAULT_LANGUAGE: Tag = Tag::new(b"dflt");

#[derive(Clone, Copy, Debug)]
pub(crate) struct TableSelection<'a> {
    pub(crate) table: LayoutTable<'a>,
    pub(crate) chosen_script: Tag,
    pub(crate) found_script: bool,
    pub(crate) lang_sys: Option<LangSys<'a>>,
}

impl<'a> TableSelection<'a> {
    pub(crate) fn select(table: LayoutTable<'a>, tags: &OpenTypeTags) -> Option<Self> {
        let scripts = table.scripts();
        let (index, chosen_script, found_script) = tags
            .scripts
            .as_slice()
            .iter()
            .find_map(|&tag| Some((scripts.index_of(tag)?, tag, true)))
            .or_else(|| {
                FALLBACK_SCRIPTS
                    .iter()
                    .find_map(|&tag| Some((scripts.index_of(tag)?, tag, false)))
            })?;
        let script = scripts.get(index)?;
        let lang_sys = tags
            .languages
            .as_slice()
            .iter()
            .chain(std::iter::once(&DEFAULT_LANGUAGE))
            .find_map(|&tag| script.lang_sys_index(tag))
            .map_or_else(
                || script.default_lang_sys(),
                |index| script.lang_sys_at(index),
            );
        Some(Self {
            table,
            chosen_script,
            found_script,
            lang_sys,
        })
    }

    pub(crate) fn find_feature(&self, tag: Tag) -> Option<u16> {
        let features = self.table.features();
        self.lang_sys?
            .feature_indices()
            .iter()
            .find(|&index| features.tag(index) == Some(tag))
    }

    pub(crate) fn required_feature(&self) -> Option<(u16, Tag)> {
        let index = self.lang_sys?.required_feature_index()?;
        Some((index, self.table.features().tag(index)?))
    }

    pub(crate) fn find_any_feature(&self, tag: Tag) -> Option<u16> {
        let features = self.table.features();
        (0..features.len())
            .map(|index| index as u16)
            .find(|&index| features.tag(index) == Some(tag))
    }
}
