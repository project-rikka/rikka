use crate::command::named;
use crate::words::spans;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    command: String,
    given: Vec<String>,
    typed: String,
}

impl Argument {
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    #[must_use]
    pub fn given(&self) -> &[String] {
        &self.given
    }

    #[must_use]
    pub fn typed(&self) -> &str {
        &self.typed
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArgumentAt {
    pub(crate) argument: Argument,
    pub(crate) at: usize,
    pub(crate) end: usize,
}

pub(crate) fn argument_at(text: &str, caret: usize) -> Option<ArgumentAt> {
    let command = named(text).filter(|name| !name.is_empty())?;
    let from = 1 + command.len();
    let region = text.get(from..)?;
    if !region.starts_with(char::is_whitespace) || caret <= from || caret > text.len() {
        return None;
    }
    let before = text.get(from..caret)?;
    let mut given = spans(before);
    let open = given
        .last()
        .is_some_and(|word| word.end == before.len())
        .then(|| given.pop())
        .flatten();

    let (typed, at, end) = match open {
        Some(word) => {
            let end = spans(region)
                .into_iter()
                .find(|whole| whole.start == word.start)
                .map_or(word.end, |whole| whole.end);
            (word.text, from + word.start, from + end)
        }
        None => (String::new(), caret, caret),
    };
    Some(ArgumentAt {
        argument: Argument {
            command: command.to_owned(),
            given: given.into_iter().map(|word| word.text).collect(),
            typed,
        },
        at,
        end,
    })
}
