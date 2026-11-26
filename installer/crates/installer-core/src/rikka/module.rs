use rikka_nix_edit::Set;
use rikka_nixos_layout::Module;

use crate::{
    answers::Answers,
    error::Result,
    rikka::{apps, boot, desktop, ime, integration, nix, packages, system},
};

pub(crate) trait RenderModule {
    fn render(self, answers: &Answers) -> Result<String>;
}

impl RenderModule for Module {
    fn render(self, answers: &Answers) -> Result<String> {
        let body = match self {
            Self::Root => return Ok(self.blank().to_string()),
            Self::Apps => apps::body(answers),
            Self::Boot => boot::body(answers)?,
            Self::Desktop => desktop::body(answers)?,
            Self::Ime => ime::body(answers),
            Self::Integration => integration::body(),
            Self::Nix => nix::body(answers),
            Self::Packages => packages::body(),
            Self::Settings => Set::new(),
            Self::System => system::body(answers),
        };
        if let Some(restored) = &answers.restored
            && let Some(text) = restored.render(self, &body, &answers.username)?
        {
            return Ok(text);
        }
        Ok(self.document(body)?.to_string())
    }
}

pub(crate) fn section(set: Set, title: &str) -> Set {
    let set = match set.entries().is_empty() {
        true => set,
        false => set.blank(),
    };
    set.comment(format!("--- {title} ---"))
}
