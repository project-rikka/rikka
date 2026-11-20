mod cloth;
mod liquid;
mod morphs;
mod output;

use std::cell::RefCell;
use std::rc::Rc;

use rikka_settings::Schema;

const DOMAIN: &str = "rikka.animate";

#[derive(Clone, Debug)]
pub(crate) struct Preferences(Rc<RefCell<rikka_settings::seam::Preferences>>);

impl Preferences {
    pub(crate) fn open() -> Self {
        Self::new(rikka_settings::seam::Preferences::open(schema()))
    }

    pub(crate) fn in_memory() -> Self {
        Self::new(rikka_settings::seam::Preferences::in_memory(schema()))
    }

    fn new(preferences: rikka_settings::seam::Preferences) -> Self {
        Self(Rc::new(RefCell::new(preferences)))
    }
}

fn schema() -> Schema {
    let schema = Schema::new(DOMAIN).summary("Animate");
    morphs::keys(cloth::keys(liquid::keys(output::keys(schema))))
}
