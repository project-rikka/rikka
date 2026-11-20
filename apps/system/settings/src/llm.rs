use rikka_llm_settings::LlmSettings;

use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;

impl Page for LlmSettings {
    fn opened(&mut self, _: &Pending, _: &Session) {
        self.open();
    }

    fn closed(&mut self) {
        self.close();
    }
}
