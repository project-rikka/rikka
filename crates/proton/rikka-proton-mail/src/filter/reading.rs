use crate::filter::sieve::{SieveArgument, SieveCommand, SieveScript, SieveTest};
use crate::filter::simple::{Comparator, Condition, ConditionKind, SimpleFilter};

const ATTACHED: &str = "x-attached";
const SEEN: &str = "\\seen";
const FLAGGED: &str = "\\flagged";

impl SimpleFilter {
    #[must_use]
    pub fn from_sieve(source: &str) -> Option<SimpleFilter> {
        SimpleFilter::from_script(&SieveScript::parse(source).ok()?)
    }

    #[must_use]
    pub fn from_script(script: &SieveScript) -> Option<SimpleFilter> {
        let mut selected: Option<&SieveCommand> = None;

        for command in script.commands() {
            match command.name() {
                "require" => {}
                "if" if is_spam_guard(command) => {}
                "if" if selected.is_none() => selected = Some(command),
                _ => return None,
            }
        }

        let command = selected?;
        let [test] = command.tests() else {
            return None;
        };

        let (mut filter, tests) = match test.name() {
            "allof" => (SimpleFilter::all(), test.tests()),
            "anyof" => (SimpleFilter::any(), test.tests()),
            "true" => (SimpleFilter::all(), &[][..]),
            _ => (SimpleFilter::all(), std::slice::from_ref(test)),
        };
        for test in tests {
            filter = filter.when(parse_condition(test)?);
        }

        for action in command.block()? {
            filter = apply_action(filter, action)?;
        }

        Some(filter)
    }
}

fn is_spam_guard(command: &SieveCommand) -> bool {
    let guarded = command.tests().iter().any(|test| {
        test.name() == "allof"
            && test
                .tests()
                .iter()
                .any(|inner| inner.name() == "environment")
    });
    let returns = command
        .block()
        .is_some_and(|block| matches!(block, [only] if only.name() == "return"));
    guarded && returns
}

fn apply_action(filter: SimpleFilter, action: &SieveCommand) -> Option<SimpleFilter> {
    match action.name() {
        "fileinto" => {
            let folder = string_arguments(action.arguments()).pop()?;
            Some(filter.file_into(folder))
        }
        "addflag" | "setflag" => {
            let mut filter = filter;
            for flag in string_arguments(action.arguments()) {
                filter = match flag.to_ascii_lowercase().as_str() {
                    SEEN => filter.mark_read(),
                    FLAGGED => filter.mark_starred(),
                    _ => return None,
                };
            }
            Some(filter)
        }
        "vacation" => {
            let mime = action
                .arguments()
                .iter()
                .any(|argument| argument.tag() == Some("mime"));
            let message = string_arguments(action.arguments()).pop()?;
            let body = match mime {
                true => message
                    .split_once("\r\n\r\n")
                    .or_else(|| message.split_once("\n\n"))
                    .map_or(message.as_str(), |(_, body)| body)
                    .to_owned(),
                false => message,
            };
            Some(filter.auto_reply(body))
        }
        "keep" | "stop" => Some(filter),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum MatchType {
    Is,
    Contains,
    Matches,
}

fn parse_condition(test: &SieveTest) -> Option<Condition> {
    match test.name() {
        "not" => {
            let [inner] = test.tests() else {
                return None;
            };
            parse_condition(inner).map(Condition::negated)
        }
        "exists" => {
            let headers = string_arguments(test.arguments());
            matches!(headers.as_slice(), [only] if only.eq_ignore_ascii_case(ATTACHED))
                .then(Condition::has_attachments)
        }
        "header" | "address" => parse_match_test(test),
        _ => None,
    }
}

fn parse_match_test(test: &SieveTest) -> Option<Condition> {
    let mut match_type = MatchType::Is;
    let mut lists: Vec<Vec<String>> = Vec::new();
    let mut arguments = test.arguments().iter();

    while let Some(argument) = arguments.next() {
        match argument {
            SieveArgument::Tag(tag) => match tag.as_str() {
                "comparator" => {
                    arguments.next();
                }
                "is" => match_type = MatchType::Is,
                "contains" => match_type = MatchType::Contains,
                "matches" => match_type = MatchType::Matches,
                "all" => {}
                _ => return None,
            },
            SieveArgument::String(single) => lists.push(vec![single.clone()]),
            SieveArgument::StringList(list) => lists.push(list.clone()),
            _ => return None,
        }
    }

    let [headers, values] = <[Vec<String>; 2]>::try_from(lists).ok()?;
    let headers: Vec<String> = headers
        .iter()
        .map(|header| header.to_ascii_lowercase())
        .collect();

    let kind = match (test.name(), headers.as_slice()) {
        ("header", [only]) if only == "subject" => ConditionKind::Subject,
        ("address", [only]) if only == "from" => ConditionKind::Sender,
        ("address", recipients)
            if !recipients.is_empty()
                && recipients
                    .iter()
                    .all(|header| matches!(header.as_str(), "to" | "cc" | "bcc")) =>
        {
            ConditionKind::Recipient
        }
        _ => return None,
    };

    let (comparator, values) = match match_type {
        MatchType::Contains => (Comparator::Contains, values),
        MatchType::Is => (Comparator::Is, values),
        MatchType::Matches => wildcard_comparator(values),
    };

    Some(Condition::new(kind, comparator, values))
}

fn wildcard_comparator(values: Vec<String>) -> (Comparator, Vec<String>) {
    let starts: Option<Vec<String>> = values
        .iter()
        .map(|value| unescape_literal(value.strip_suffix('*')?))
        .collect();
    if let Some(starts) = starts {
        return (Comparator::Starts, starts);
    }

    let ends: Option<Vec<String>> = values
        .iter()
        .map(|value| unescape_literal(value.strip_prefix('*')?))
        .collect();
    if let Some(ends) = ends {
        return (Comparator::Ends, ends);
    }

    (Comparator::Matches, values)
}

fn unescape_literal(pattern: &str) -> Option<String> {
    let mut plain = String::with_capacity(pattern.len());
    let mut chars = pattern.chars();
    while let Some(character) = chars.next() {
        match character {
            '\\' => plain.push(chars.next()?),
            '*' | '?' => return None,
            other => plain.push(other),
        }
    }
    Some(plain)
}

fn string_arguments(arguments: &[SieveArgument]) -> Vec<String> {
    arguments
        .iter()
        .filter_map(SieveArgument::strings)
        .flatten()
        .map(str::to_owned)
        .collect()
}
