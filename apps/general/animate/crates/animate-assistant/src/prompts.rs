use rikka_prompt::{Facts, Prompt};

use crate::{look, vocabulary};

pub(crate) const SYSTEM: &str = include_str!("../../../prompts/system.md");
const GUIDE: &str = include_str!("../../../prompts/look.md");

pub(crate) fn facts(facts: &mut Facts) -> rikka_prompt::Result<()> {
    facts.set("world_ports", look::world_ports());
    facts.set("material_presets", look::presets());
    facts.set("joints", vocabulary::JointTable.to_string());
    facts.set("curves", vocabulary::curves());
    facts.set("tags", vocabulary::tags());
    facts.set("parts", vocabulary::parts());
    facts.set("hands", vocabulary::hands());
    facts.set("expressions", vocabulary::expressions());
    facts.set("presets", vocabulary::presets());
    facts.set("feels", vocabulary::knobs());
    facts.set("wind_settings", vocabulary::wind_ranges());
    let guide = Prompt::read("guide", GUIDE)?.body(facts)?;
    facts.set("look", guide);
    Ok(())
}
