use rikka_prompt::Facts;

use crate::framing::{LONGEST, MAX_SIDE};
use crate::ops;
use crate::vocabulary::{PROFILES, formats, listed, names};

pub(crate) const SYSTEM: &str = include_str!("../../../prompts/system.md");

pub(crate) fn facts(facts: &mut Facts) -> rikka_prompt::Result<()> {
    facts.set("operations", ops::table());
    facts.set("profiles", listed(names(&PROFILES)));
    facts.set("formats", formats());
    facts.set("longest", LONGEST.to_string());
    facts.set("most_side", MAX_SIDE.to_string());
    Ok(())
}
