use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use browser_preferences::Preferences;

use crate::browser::Browser;
use crate::startup::Startup;
use browser_model::Geometry;

pub(crate) struct Boot {
    waiting: RefCell<Option<(Preferences, Startup)>>,
    browser: OnceCell<Rc<Browser>>,
}

impl Boot {
    pub(crate) fn begin() -> Rc<Boot> {
        let preferences = Preferences::open();
        let startup = Startup::begin();
        Rc::new(Boot {
            waiting: RefCell::new(Some((preferences, startup))),
            browser: OnceCell::new(),
        })
    }

    pub(crate) fn first_geometry(&self) -> Option<Geometry> {
        self.waiting
            .borrow()
            .as_ref()
            .and_then(|(_, startup)| startup.first_geometry())
    }

    pub(crate) fn ready(&self) -> Option<&Rc<Browser>> {
        self.browser.get()
    }

    pub(crate) fn browser(&self) -> Rc<Browser> {
        Rc::clone(self.browser.get_or_init(|| {
            let (preferences, startup) = self
                .waiting
                .borrow_mut()
                .take()
                .expect("browser is initialized once");
            let (store, loaded) = startup.finish();
            Browser::new(preferences, store, &loaded)
        }))
    }
}
