use rikka_mcp::{Prompt, PromptArgument, Resource, ResourceTemplate};
use rikka_ui::{Label, MaterialSymbol};

use crate::builtin::Builtin;
use crate::chat::Labels;
use crate::command::written_command;
use crate::mention::{opened_mention, written_mention};
use crate::words::{opened, quoted};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    symbol: MaterialSymbol,
    look: Look,
    spelt: String,
    insertion: String,
    narrowing: Narrowing,
    runs: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Look {
    Path {
        under: String,
        leaf: String,
    },
    Named {
        name: String,
        hint: Option<Label>,
        about: Label,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Narrowing {
    Sigil(char),
    Argument,
}

impl Offer {
    #[must_use]
    pub fn path(path: impl Into<String>) -> Offer {
        let path = path.into();
        let symbol = match path.ends_with('/') {
            true => MaterialSymbol::FOLDER,
            false => MaterialSymbol::DESCRIPTION,
        };
        let leaf = leaf(&path).to_owned();
        Offer {
            symbol,
            look: Look::Path {
                under: path[..path.len() - leaf.len()].to_owned(),
                leaf,
            },
            insertion: written_mention(&path),
            spelt: path,
            narrowing: Narrowing::Sigil('@'),
            runs: false,
        }
    }

    #[must_use]
    pub fn builtin<C: Labels>(builtin: Builtin) -> Offer {
        let mut offer = Offer::command(
            builtin.name(),
            Some(builtin.hint::<C>()),
            builtin.description::<C>(),
        );
        offer.symbol = builtin.symbol();
        offer.runs = true;
        offer
    }

    #[must_use]
    pub fn command(name: &str, hint: Option<Label>, about: Label) -> Offer {
        Offer {
            symbol: MaterialSymbol::KEYBOARD_COMMAND_KEY,
            look: Look::Named {
                name: format!("/{name}"),
                hint,
                about,
            },
            spelt: name.to_owned(),
            insertion: written_command(name),
            narrowing: Narrowing::Sigil('/'),
            runs: false,
        }
    }

    #[must_use]
    pub fn prompt(prompt: &Prompt) -> Offer {
        let hint = (!prompt.arguments().is_empty()).then(|| Label::from(hint(prompt.arguments())));
        let about = gist(prompt.description().or(prompt.title()));
        let mut offer = Offer::command(prompt.command(), hint, about);
        offer.symbol = MaterialSymbol::EXTENSION;
        offer.runs = prompt.arguments().is_empty();
        offer
    }

    #[must_use]
    pub fn resource(resource: &Resource) -> Offer {
        Offer {
            symbol: MaterialSymbol::DATA_OBJECT,
            look: Look::Named {
                name: resource.title().unwrap_or(resource.name()).to_owned(),
                hint: Some(Label::from(format!("@{}", resource.mention()))),
                about: gist(resource.description()),
            },
            spelt: resource.mention().to_owned(),
            insertion: written_mention(resource.mention()),
            narrowing: Narrowing::Sigil('@'),
            runs: false,
        }
    }

    #[must_use]
    pub fn template(template: &ResourceTemplate) -> Offer {
        let prefix = template.mention_prefix();
        Offer {
            symbol: MaterialSymbol::DATA_OBJECT,
            look: Look::Named {
                name: template.title().unwrap_or(template.name()).to_owned(),
                hint: Some(Label::from(format!("@{}", template.mention_template()))),
                about: gist(template.description()),
            },
            insertion: opened_mention(prefix),
            spelt: prefix.to_owned(),
            narrowing: Narrowing::Sigil('@'),
            runs: false,
        }
    }

    #[must_use]
    pub fn value(value: &str, argument: &PromptArgument) -> Offer {
        let about = gist(argument.description().or(argument.title()));
        Offer {
            symbol: MaterialSymbol::SHORT_TEXT,
            look: Look::Named {
                name: value.to_owned(),
                hint: Some(Label::from(argument.name())),
                about,
            },
            spelt: value.to_owned(),
            insertion: format!("{} ", quoted(value)),
            narrowing: Narrowing::Argument,
            runs: false,
        }
    }

    #[must_use]
    pub fn mentioned(
        symbol: MaterialSymbol,
        name: &str,
        title: impl Into<String>,
        about: impl Into<Label>,
    ) -> Offer {
        Offer {
            symbol,
            look: Look::Named {
                name: title.into(),
                hint: Some(Label::from(format!("@{name}"))),
                about: about.into(),
            },
            spelt: name.to_owned(),
            insertion: format!("@{name} "),
            narrowing: Narrowing::Sigil('@'),
            runs: false,
        }
    }

    #[must_use]
    pub fn symbol(&self) -> MaterialSymbol {
        self.symbol
    }

    #[must_use]
    pub(crate) fn look(&self) -> &Look {
        &self.look
    }

    #[must_use]
    pub fn insertion(&self) -> &str {
        &self.insertion
    }

    #[must_use]
    pub fn spelt(&self) -> &str {
        &self.spelt
    }

    #[must_use]
    pub fn runs(&self) -> bool {
        self.runs
    }

    #[must_use]
    pub fn narrowed(&self, agreed: &str) -> String {
        match self.narrowing {
            Narrowing::Sigil('@') => opened_mention(agreed),
            Narrowing::Sigil(sigil) => format!("{sigil}{agreed}"),
            Narrowing::Argument => opened(agreed),
        }
    }
}

fn hint(arguments: &[PromptArgument]) -> String {
    arguments
        .iter()
        .map(|argument| match argument.is_required() {
            true => format!("<{}>", argument.name()),
            false => format!("[{}]", argument.name()),
        })
        .collect::<Vec<String>>()
        .join(" ")
}

pub(crate) fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

fn gist(text: Option<&str>) -> Label {
    Label::from(text.and_then(first_line).unwrap_or_default().to_owned())
}

fn leaf(path: &str) -> &str {
    let end = path.strip_suffix('/').unwrap_or(path);
    match end.rfind('/') {
        Some(at) => &path[at + 1..],
        None => path,
    }
}
