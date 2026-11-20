use rikka_text_unicode::{EmojiPresentation, Script};

use crate::locale::Locale;
use crate::matching::FaceMatch;

#[derive(Clone, Copy, Debug)]
pub struct FallbackRequest<'a> {
    pub(crate) primary: FaceMatch,
    pub(crate) cluster: &'a str,
    pub(crate) script: Script,
    pub(crate) locale: Option<Locale>,
    pub(crate) presentation: Option<EmojiPresentation>,
}

impl<'a> FallbackRequest<'a> {
    pub fn new(primary: FaceMatch, cluster: &'a str) -> Self {
        Self {
            primary,
            cluster,
            script: Script::Common,
            locale: None,
            presentation: None,
        }
    }

    #[must_use]
    pub fn script(mut self, script: Script) -> Self {
        self.script = script;
        self
    }

    #[must_use]
    pub fn locale(mut self, locale: impl Into<Option<Locale>>) -> Self {
        self.locale = locale.into();
        self
    }

    #[must_use]
    pub fn presentation(mut self, presentation: impl Into<Option<EmojiPresentation>>) -> Self {
        self.presentation = presentation.into();
        self
    }
}
