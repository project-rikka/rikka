use rikka_text_font::Tag;
use rikka_text_unicode::Script;

use crate::cjk::CjkRegion;
use crate::face::Face;

pub(crate) const NAMED: u8 = 2;
pub(crate) const DECLARED: u8 = 1;
pub(crate) const UNRELATED: u8 = 0;

pub(crate) struct ScriptAffinity {
    name: Option<String>,
    tags: Vec<Tag>,
    cjk: bool,
}

impl ScriptAffinity {
    pub(crate) fn new(script: Script) -> Self {
        let cjk = CjkRegion::from_script(script).is_some();
        let generic = matches!(script, Script::Common | Script::Inherited | Script::Unknown);
        Self {
            name: (!generic && !cjk).then(|| format!("{script:?}").to_ascii_lowercase()),
            tags: script.opentype_tags().iter().map(Tag::new).collect(),
            cjk,
        }
    }

    pub(crate) fn is_specific(&self) -> bool {
        self.cjk || self.name.is_some()
    }

    pub(crate) fn score(&self, face: &Face) -> u8 {
        let named = if self.cjk {
            face.cjk_region.is_some()
        } else {
            self.name.as_ref().is_some_and(|name| {
                face.folded_families
                    .iter()
                    .any(|family| family.contains(name.as_str()))
            })
        };
        if named {
            NAMED
        } else if self
            .tags
            .iter()
            .any(|tag| face.record.scripts.contains(tag))
        {
            DECLARED
        } else {
            UNRELATED
        }
    }
}
