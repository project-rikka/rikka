use crate::prelude::*;

use crate::llm_settings::LlmSettings;

#[derive(Debug)]
pub(crate) struct Language {
    language: TextDocument,
}

impl Language {
    pub(crate) fn new(llm: &rikka_llm_accounts::Llm) -> Language {
        Language {
            language: TextDocument::new(llm.language().unwrap_or_default()),
        }
    }

    pub(crate) fn reread(&mut self, llm: &rikka_llm_accounts::Llm) {
        self.language = TextDocument::new(llm.language().unwrap_or_default());
    }

    pub(crate) fn commit(&self, llm: &mut rikka_llm_accounts::Llm) {
        llm.set_language(self.language.text().trim());
    }
}

pub(crate) fn view(language: &Language) -> impl View {
    let commit =
        Handler::new(|settings: &mut LlmSettings| settings.language.commit(&mut settings.llm));

    section(
        tr!("llm-settings.language"),
        text_field(&language.language)
            .placeholder(tr!("llm-settings.language-any"))
            .on_change(|| {})
            .on_blur(commit.clone())
            .on_submit(commit)
            .fill_width(),
    )
}
