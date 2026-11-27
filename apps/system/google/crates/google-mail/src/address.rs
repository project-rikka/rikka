use std::fmt::Write;

use crate::error::{Error, Result};
use crate::words;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Address {
    pub name: String,
    pub email: String,
}

impl Address {
    #[must_use]
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Address {
        Address {
            name: name.into(),
            email: email.into(),
        }
    }

    #[must_use]
    pub fn parse_list(header: &str) -> Vec<Address> {
        split(header)
            .into_iter()
            .filter_map(|mailbox| Address::parse(&mailbox))
            .collect()
    }

    #[must_use]
    pub fn parse(mailbox: &str) -> Option<Address> {
        let mailbox = mailbox.trim();
        let (name, email) = match (mailbox.rfind('<'), mailbox.rfind('>')) {
            (Some(open), Some(close)) if open < close => {
                (&mailbox[..open], &mailbox[open + 1..close])
            }
            _ => ("", mailbox),
        };
        let email = email.trim();
        if email.is_empty() && name.trim().is_empty() {
            return None;
        }
        Some(Address {
            name: words::decode(&unquoted(name)),
            email: email.to_owned(),
        })
    }

    pub fn typed_list(typed: &str) -> Result<Vec<Address>> {
        Address::parse_list(typed)
            .into_iter()
            .map(|address| match address.is_deliverable() {
                true => Ok(address),
                false => Err(Error::Address(address.email)),
            })
            .collect()
    }

    #[must_use]
    pub fn label(&self) -> &str {
        match self.name.is_empty() {
            true => &self.email,
            false => &self.name,
        }
    }

    #[must_use]
    pub fn short(&self) -> &str {
        match self.name.is_empty() {
            true => self.email.split('@').next().unwrap_or(&self.email),
            false => &self.name,
        }
    }

    #[must_use]
    pub fn is_deliverable(&self) -> bool {
        let Some((local, domain)) = self.email.rsplit_once('@') else {
            return false;
        };
        !local.is_empty()
            && domain.contains('.')
            && !self
                .email
                .contains(|letter: char| letter.is_whitespace() || letter.is_control())
    }

    pub(crate) fn header(&self) -> String {
        match self.name.is_empty() {
            true => self.email.clone(),
            false => format!("{} <{}>", words::phrase(&self.name), self.email),
        }
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.name.is_empty() {
            true => formatter.write_str(&self.email),
            false => write!(formatter, "{} <{}>", self.name, self.email),
        }
    }
}

pub(crate) fn header_list(addresses: &[Address]) -> String {
    let mut list = String::new();
    for (index, address) in addresses.iter().enumerate() {
        if index > 0 {
            list.push_str(",\r\n ");
        }
        write!(list, "{}", address.header()).expect("strings accept text");
    }
    list
}

fn split(header: &str) -> Vec<String> {
    let mut mailboxes = Vec::new();
    let mut current = String::new();
    let (mut quoted, mut escaped, mut angled, mut comment) = (false, false, false, 0_u32);
    for letter in header.chars() {
        if escaped {
            escaped = false;
            current.push(letter);
            continue;
        }
        match letter {
            '\\' if quoted || comment > 0 => {
                escaped = true;
                current.push(letter);
            }
            '"' if comment == 0 => {
                quoted = !quoted;
                current.push(letter);
            }
            '(' if !quoted => comment += 1,
            ')' if !quoted && comment > 0 => comment -= 1,
            _ if comment > 0 => {}
            '<' if !quoted => {
                angled = true;
                current.push(letter);
            }
            '>' if !quoted => {
                angled = false;
                current.push(letter);
            }
            ',' | ';' if !quoted && !angled => mailboxes.push(std::mem::take(&mut current)),
            _ => current.push(letter),
        }
    }
    mailboxes.push(current);
    mailboxes
        .into_iter()
        .filter(|mailbox| !mailbox.trim().is_empty())
        .collect()
}

fn unquoted(name: &str) -> String {
    let name = name.trim();
    let Some(inner) = name
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return name.to_owned();
    };
    let mut plain = String::with_capacity(inner.len());
    let mut escaped = false;
    for letter in inner.chars() {
        if letter == '\\' && !escaped {
            escaped = true;
        } else {
            escaped = false;
            plain.push(letter);
        }
    }
    plain
}
