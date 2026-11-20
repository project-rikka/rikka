use rikka_prompt::{Facts, Prompt, Result};
use rikka_types::Choice;

use crate::language::Language;
use crate::profile::Profile;

const TEXT: &str = include_str!("../prompts/text.md");
const IMAGE: &str = include_str!("../prompts/image.md");

const NATURAL: &str = include_str!("../prompts/profiles/natural.md");
const CASUAL: &str = include_str!("../prompts/profiles/casual.md");
const POLITE: &str = include_str!("../prompts/profiles/polite.md");
const BUSINESS: &str = include_str!("../prompts/profiles/business.md");

const TARGET: &str = "target_language";
const SOURCE: &str = "source_language";
const PROFILE: &str = "profile";
const UNKNOWN: &str = "unknown_source";
const MAX_CHARACTERS: &str = "max_characters";

#[derive(Clone, Debug)]
pub(crate) struct Prompts {
    text: Prompt,
    image: Prompt,
    profiles: Profiles,
}

#[derive(Clone, Debug)]
struct Profiles {
    natural: Prompt,
    casual: Prompt,
    polite: Prompt,
    business: Prompt,
}

impl Prompts {
    pub(crate) fn load() -> Result<Prompts> {
        Ok(Prompts {
            text: Prompt::read("text", TEXT)?,
            image: Prompt::read("image", IMAGE)?,
            profiles: Profiles::load()?,
        })
    }

    pub(crate) fn text(
        &self,
        target: &Language,
        source: &Language,
        profile: Profile,
    ) -> Result<String> {
        let mut facts = languages(&self.text, target, source)?;
        let styled = self.profiles.for_profile(profile).body(&facts)?;
        facts.set(PROFILE, styled);
        self.text.body(&facts)
    }

    pub(crate) fn image(&self, target: &Language, source: &Language) -> Result<String> {
        self.image.body(&languages(&self.image, target, source)?)
    }

    pub(super) fn max_characters(&self) -> Result<usize> {
        Ok(self.text.limit(MAX_CHARACTERS)? as usize)
    }
}

impl Profiles {
    fn load() -> Result<Profiles> {
        Ok(Profiles {
            natural: read(Profile::Natural, NATURAL)?,
            casual: read(Profile::Casual, CASUAL)?,
            polite: read(Profile::Polite, POLITE)?,
            business: read(Profile::Business, BUSINESS)?,
        })
    }

    fn for_profile(&self, profile: Profile) -> &Prompt {
        match profile {
            Profile::Natural => &self.natural,
            Profile::Casual => &self.casual,
            Profile::Polite => &self.polite,
            Profile::Business => &self.business,
        }
    }
}

fn read(profile: Profile, text: &str) -> Result<Prompt> {
    Prompt::read(&format!("profiles/{}", profile.slug()), text)
}

fn languages(prompt: &Prompt, target: &Language, source: &Language) -> Result<Facts> {
    let mut facts = Facts::new();
    facts.set(TARGET, target.named().unwrap_or_default());
    facts.set(SOURCE, spoken(prompt, source)?);
    Ok(facts)
}

fn spoken(prompt: &Prompt, source: &Language) -> Result<String> {
    match source.named() {
        Some(named) => Ok(named.to_owned()),
        None => prompt.param(UNKNOWN, &Facts::new()),
    }
}
