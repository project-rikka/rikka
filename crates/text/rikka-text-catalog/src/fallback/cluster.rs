use rikka_text_unicode::{
    EmojiPresentation, GeneralCategory, Script, emoji_presentation, properties,
};

use crate::face::Face;
use crate::fallback::request::FallbackRequest;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Cluster<'a> {
    pub(crate) text: &'a str,
    pub(crate) script: Script,
    pub(crate) presentation: EmojiPresentation,
}

impl<'a> Cluster<'a> {
    pub(crate) fn new(request: &FallbackRequest<'a>) -> Self {
        let text = request.cluster;
        Self {
            text,
            script: if is_specific(request.script) {
                request.script
            } else {
                derived_script(text)
            },
            presentation: request
                .presentation
                .or_else(|| emoji_presentation(text))
                .unwrap_or(EmojiPresentation::Text),
        }
    }

    pub(crate) fn required(&self) -> impl Iterator<Item = char> + use<'a> {
        self.text
            .chars()
            .filter(|&character| !is_invisible(character))
    }

    #[inline]
    pub(crate) fn covered_by(&self, face: &Face) -> bool {
        self.required().all(|character| face.covers(character))
    }
}

fn is_specific(script: Script) -> bool {
    !matches!(script, Script::Common | Script::Inherited | Script::Unknown)
}

fn derived_script(text: &str) -> Script {
    text.chars()
        .map(|character| properties(character).script())
        .find(|&script| is_specific(script))
        .unwrap_or(Script::Common)
}

fn is_invisible(character: char) -> bool {
    let properties = properties(character);
    properties.is_default_ignorable()
        || matches!(
            properties.general_category(),
            GeneralCategory::Control
                | GeneralCategory::LineSeparator
                | GeneralCategory::ParagraphSeparator
        )
}
