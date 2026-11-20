use rikka_network::Invitation;

use crate::link::Link;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Meaning {
    Link(Link),
    Network(Invitation),
    Text,
}

#[must_use]
pub fn meaning(text: &str) -> Meaning {
    if let Some(link) = Link::read(text) {
        return Meaning::Link(link);
    }
    match text.parse::<Invitation>() {
        Ok(invitation) => Meaning::Network(invitation),
        Err(_) => Meaning::Text,
    }
}
