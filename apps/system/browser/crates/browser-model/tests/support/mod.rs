use browser_model::{LivePage, Loaded, Model, Names};
use browser_store::Store;

#[derive(Debug)]
pub(crate) struct FakePage(pub(crate) u32);

impl LivePage for FakePage {
    type View = u32;

    fn view(&self) -> &u32 {
        &self.0
    }
}

fn name() -> String {
    "Default".to_owned()
}

fn numbered(number: usize) -> String {
    format!("Default {number}")
}

const NAMES: Names = Names {
    profile: name,
    numbered_profile: numbered,
    space: name,
    numbered_space: numbered,
    folder: name,
};

pub(crate) fn model() -> Model<FakePage> {
    let store = Store::in_memory().expect("in-memory store");
    let loaded = Loaded::read(&store).expect("empty store reads");
    Model::load(store, &loaded, NAMES)
}
