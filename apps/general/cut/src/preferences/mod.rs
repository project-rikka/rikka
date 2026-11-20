mod export;
mod timeline;

use rikka_settings::{Key, Schema};

const DOMAIN: &str = "rikka.cut";

#[derive(Debug)]
pub(crate) struct Preferences(rikka_settings::seam::Preferences);

impl Preferences {
    pub(crate) fn open() -> Preferences {
        Preferences(rikka_settings::seam::Preferences::open(schema()))
    }
}

fn schema() -> Schema {
    let schema = Schema::new(DOMAIN).summary("Cut");
    timeline::keys(export::keys(schema))
}

fn choice<T: Copy + PartialEq>(name: &str, table: &[(T, &'static str)], default: T) -> Key {
    Key::string(name, name_of(table, default))
        .choices(table.iter().map(|(_, name)| (*name).to_owned()))
}

fn named<T: Copy>(table: &[(T, &str)], name: &str) -> Option<T> {
    table
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(value, _)| *value)
}

fn name_of<T: Copy + PartialEq>(table: &[(T, &'static str)], value: T) -> &'static str {
    table
        .iter()
        .find(|(candidate, _)| *candidate == value)
        .map_or(table[0].1, |(_, name)| name)
}
