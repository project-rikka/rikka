use super::{Look, known};
use crate::words::exact_segments;

const SAYS_NOTHING: &[&str] = &["echo"];

pub(crate) fn look(command: &str) -> Option<Look> {
    let mut looking: Option<Look> = None;
    let mut moved: Option<String> = None;

    for words in exact_segments(command)? {
        let (name, arguments) = words.split_first()?;

        if name == "cd" {
            moved = moved.or_else(|| {
                arguments
                    .first()
                    .filter(|folder| !folder.starts_with('-'))
                    .cloned()
            });
            continue;
        }
        if SAYS_NOTHING.contains(&name.as_str()) {
            continue;
        }

        let look = known::read(name, arguments)?;
        looking = Some(match looking {
            Some(previous) => previous.and(look),
            None => look,
        });
    }

    let mut looking = looking?;
    if looking.subject.is_empty() {
        looking.subject = moved.unwrap_or_default();
    }
    Some(looking)
}
