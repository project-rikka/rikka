use crate::address::MailAddress;
use crate::mime::encoded_words;

#[derive(Default)]
struct PendingAddress {
    phrase: String,
    comment: Option<String>,
    angle: Option<String>,
}

impl PendingAddress {
    fn finish(&mut self, group: Option<&str>, addresses: &mut Vec<MailAddress>) {
        let pending = std::mem::take(self);
        let phrase = collapse_whitespace(&pending.phrase);

        let (address, name) = match pending.angle {
            Some(angle) => (angle.trim().to_owned(), phrase),
            None => (
                phrase.replace(' ', ""),
                pending
                    .comment
                    .map(|comment| collapse_whitespace(&comment))
                    .unwrap_or_default(),
            ),
        };

        if address.is_empty() {
            return;
        }

        let name = encoded_words::decode(&name);
        let address = MailAddress::with_name(address, name.trim());
        addresses.push(match group {
            Some(group) => address.in_group(group),
            None => address,
        });
    }
}

pub(crate) fn parse(value: &str) -> Vec<MailAddress> {
    let mut addresses = Vec::new();
    let mut pending = PendingAddress::default();
    let mut group: Option<String> = None;
    let mut chars = value.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '"' => {
                let mut quoted = String::new();
                while let Some(inner) = chars.next() {
                    match inner {
                        '\\' => {
                            if let Some(escaped) = chars.next() {
                                quoted.push(escaped);
                            }
                        }
                        '"' => break,
                        other => quoted.push(other),
                    }
                }
                pending.phrase.push_str(&quoted);
            }
            '(' => {
                let mut depth = 1usize;
                let mut comment = String::new();
                for inner in chars.by_ref() {
                    match inner {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    comment.push(inner);
                }
                pending.comment.get_or_insert(comment);
            }
            '<' => {
                let mut angle = String::new();
                for inner in chars.by_ref() {
                    if inner == '>' {
                        break;
                    }
                    angle.push(inner);
                }
                pending.angle = Some(angle);
            }
            ',' => pending.finish(group.as_deref(), &mut addresses),
            ':' if pending.angle.is_none() && group.is_none() => {
                group = Some(encoded_words::decode(&collapse_whitespace(&pending.phrase)));
                pending = PendingAddress::default();
            }
            ';' => {
                pending.finish(group.as_deref(), &mut addresses);
                group = None;
            }
            other => pending.phrase.push(other),
        }
    }

    pending.finish(group.as_deref(), &mut addresses);
    addresses
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
