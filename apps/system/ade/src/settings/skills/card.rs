use ade_skill::Skill;
use rikka_app::prelude::*;

use crate::settings::rows::entry;

pub(crate) fn card(skill: &Skill) -> impl View {
    entry(skill.name().to_owned(), skill.scope(), skill.location())
}
