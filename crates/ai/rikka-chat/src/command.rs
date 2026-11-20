#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandName {
    query: String,
}

impl CommandName {
    pub(crate) fn end(&self) -> usize {
        1 + self.query.len()
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invoked<'a> {
    name: &'a str,
    arguments: &'a str,
}

impl<'a> Invoked<'a> {
    pub fn name(&self) -> &'a str {
        self.name
    }

    pub fn arguments(&self) -> &'a str {
        self.arguments
    }
}

pub(crate) fn command_at(text: &str, caret: usize) -> Option<CommandName> {
    let typed = named(text)?;
    (caret <= 1 + typed.len()).then(|| CommandName {
        query: typed.to_owned(),
    })
}

pub fn invoked(text: &str) -> Option<Invoked<'_>> {
    let name = named(text).filter(|name| !name.is_empty())?;
    Some(Invoked {
        name,
        arguments: text[1 + name.len()..].trim(),
    })
}

pub(crate) fn written_command(name: &str) -> String {
    format!("/{name} ")
}

fn is_name(letter: char) -> bool {
    letter.is_alphanumeric() || matches!(letter, '-' | '_' | ':')
}

pub(crate) fn named(text: &str) -> Option<&str> {
    let after = text.strip_prefix('/')?;
    let end = after.find(|letter| !is_name(letter)).unwrap_or(after.len());
    Some(&after[..end])
}
