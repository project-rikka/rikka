use crate::shell::Shell;
use browser_model::TabId;

impl Shell {
    pub(crate) fn translation_started(&mut self, tab: TabId) {
        self.browser().translations().tabs_mut().navigated(tab);
        if self.address().translate_popover() == Some(tab) {
            self.close_translate_popover();
        }
    }

    pub(crate) fn prune_translations(&mut self) {
        let browser = self.browser();
        let model = browser.model();
        browser.translations().tabs_mut().retain(|tab, view| {
            model
                .tab(tab)
                .and_then(|tab_entry| tab_entry.page().view())
                .is_some_and(|live| live == view)
        });
    }
}
