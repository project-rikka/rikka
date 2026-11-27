use browser_model::Names;
use rikka_app::prelude::*;

use crate::page::Live;

pub(crate) type Model = browser_model::Model<Live>;
pub(crate) type Tab = browser_model::Tab<Live>;

pub(crate) const NAMES: Names = Names {
    profile: default_profile,
    numbered_profile,
    space: default_space,
    numbered_space,
    folder: new_folder,
};

fn default_profile() -> String {
    tr!("browser.profile.default").to_string()
}

fn numbered_profile(number: usize) -> String {
    tr!("browser.profile.numbered", number = &number.to_string()).to_string()
}

fn default_space() -> String {
    tr!("browser.space.default").to_string()
}

fn numbered_space(number: usize) -> String {
    tr!("browser.space.numbered", number = &number.to_string()).to_string()
}

fn new_folder() -> String {
    tr!("browser.folder.new").to_string()
}
