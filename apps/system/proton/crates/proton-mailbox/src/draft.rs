use rikka_proton_account::AddressId;
use rikka_proton_mail::{DraftAction, MessageId};

use crate::opened::OpenedMessage;
use crate::person::Person;

const REPLIED: &str = "Re: ";
const FORWARDED: &str = "Fw: ";

#[derive(Clone)]
pub struct Attaching {
    pub name: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

impl Attaching {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        media_type: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Attaching {
        Attaching {
            name: name.into(),
            media_type: media_type.into(),
            bytes,
        }
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.bytes.len()
    }
}

impl std::fmt::Debug for Attaching {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Attaching")
            .field("name", &self.name)
            .field("media_type", &self.media_type)
            .field("size", &self.bytes.len())
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answering {
    pub message: MessageId,
    pub action: DraftAction,
}

#[derive(Clone, Debug, Default)]
pub struct Draft {
    pub id: Option<MessageId>,
    pub from: Option<AddressId>,
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub body: String,
    pub answering: Option<Answering>,
    pub files: Vec<Attaching>,
}

impl Draft {
    #[must_use]
    pub fn blank(from: Option<AddressId>) -> Draft {
        Draft {
            from,
            ..Draft::default()
        }
    }

    #[must_use]
    pub fn replying(
        opened: &OpenedMessage,
        everyone: bool,
        from: Option<AddressId>,
        header: &str,
    ) -> Draft {
        let mut cc = Vec::new();
        if everyone {
            cc.extend(opened.to.iter().chain(&opened.cc).cloned());
        }
        let answering = opened.answering();
        cc.retain(|person| {
            !answering
                .iter()
                .any(|recipient| recipient.address.eq_ignore_ascii_case(&person.address))
        });

        Draft {
            from: from.or_else(|| Some(opened.address.clone())),
            to: address_list(&answering),
            cc: address_list(&cc),
            subject: prefixed(REPLIED, &opened.subject),
            body: body(header, opened.words.text()),
            answering: Some(Answering {
                message: opened.id.clone(),
                action: match everyone {
                    true => DraftAction::ReplyAll,
                    false => DraftAction::Reply,
                },
            }),
            ..Draft::default()
        }
    }

    #[must_use]
    pub fn forwarding(opened: &OpenedMessage, from: Option<AddressId>, header: &str) -> Draft {
        Draft {
            from: from.or_else(|| Some(opened.address.clone())),
            subject: prefixed(FORWARDED, &opened.subject),
            body: body(header, opened.words.text()),
            answering: Some(Answering {
                message: opened.id.clone(),
                action: DraftAction::Forward,
            }),
            ..Draft::default()
        }
    }

    #[must_use]
    pub fn editing(opened: &OpenedMessage) -> Draft {
        Draft {
            id: Some(opened.id.clone()),
            from: Some(opened.address.clone()),
            to: address_list(&opened.to),
            cc: address_list(&opened.cc),
            bcc: address_list(&opened.bcc),
            subject: opened.subject.clone(),
            body: opened.words.text().to_string(),
            answering: None,
            files: Vec::new(),
        }
    }

    #[must_use]
    pub fn recipients(&self) -> Vec<String> {
        let mut all = addresses(&self.to);
        all.extend(addresses(&self.cc));
        all.extend(addresses(&self.bcc));
        all
    }

    #[must_use]
    pub fn is_sendable(&self) -> bool {
        self.from.is_some() && !self.recipients().is_empty()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.to.trim().is_empty()
            && self.cc.trim().is_empty()
            && self.bcc.trim().is_empty()
            && self.subject.trim().is_empty()
            && self.body.trim().is_empty()
            && self.files.is_empty()
    }
}

#[must_use]
pub fn addresses(text: &str) -> Vec<String> {
    let mut parsed = Vec::new();

    for piece in text.split([',', ';', '\n']) {
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        let address = match (piece.rfind('<'), piece.rfind('>')) {
            (Some(from), Some(to)) if to > from => piece[from + 1..to].trim(),
            _ => piece,
        };
        if !address.contains('@') {
            continue;
        }
        let address = address.to_owned();
        if !parsed
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&address))
        {
            parsed.push(address);
        }
    }

    parsed
}

#[must_use]
pub fn quoted(text: &str) -> String {
    text.lines()
        .map(|line| match line.trim().is_empty() {
            true => ">".to_owned(),
            false => format!("> {line}"),
        })
        .collect::<Vec<String>>()
        .join("\n")
}

fn body(header: &str, text: &str) -> String {
    format!("\n\n{header}\n\n{}\n", quoted(text))
}

fn address_list(people: &[Person]) -> String {
    people
        .iter()
        .map(Person::formatted)
        .collect::<Vec<String>>()
        .join(", ")
}

fn prefixed(prefix: &str, subject: &str) -> String {
    let subject = subject.trim();
    match subject
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
    {
        true => subject.to_owned(),
        false => format!("{prefix}{subject}"),
    }
}
