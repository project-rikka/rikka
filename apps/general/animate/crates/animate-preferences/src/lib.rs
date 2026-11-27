mod cloth;
mod library;
mod liquid;
mod morphs;
mod output;

use std::cell::RefCell;
use std::rc::Rc;

use rikka_settings::Schema;

const DOMAIN: &str = "rikka.animate";

#[derive(Clone, Debug)]
pub struct Preferences(Rc<RefCell<rikka_settings::seam::Preferences>>);

impl Preferences {
    #[must_use]
    pub fn open() -> Self {
        Self::new(rikka_settings::seam::Preferences::open(Self::schema()))
    }

    #[must_use]
    pub fn in_memory() -> Self {
        Self::new(rikka_settings::seam::Preferences::in_memory(Self::schema()))
    }

    #[must_use]
    pub fn schema() -> Schema {
        let schema = Schema::new(DOMAIN).summary("Animate");
        library::keys(morphs::keys(cloth::keys(liquid::keys(output::keys(
            schema,
        )))))
    }

    fn new(preferences: rikka_settings::seam::Preferences) -> Self {
        Self(Rc::new(RefCell::new(preferences)))
    }
}
