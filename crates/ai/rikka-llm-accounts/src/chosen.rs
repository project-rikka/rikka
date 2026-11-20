use rikka_llm::ModelInfo;

use crate::llm::Llm;
use crate::selection::Selection;

impl Llm {
    #[must_use]
    pub fn model_of(&self, selection: Option<&Selection>) -> Option<&ModelInfo> {
        match selection {
            Some(selection) => selection.model_info(self),
            None => self.selected_account()?.model_info(),
        }
    }

    #[must_use]
    pub fn window_of(&self, selection: Option<&Selection>) -> u64 {
        self.model_of(selection).map_or(0, ModelInfo::context)
    }
}
