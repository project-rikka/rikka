use crate::arguments;

const ARGUMENTS: &str = "$ARGUMENTS";

pub(crate) fn fill(body: &str, arguments: &str) -> String {
    let words = arguments::words(arguments);
    let (filled, named) = substitute(body, arguments, &words);

    let trailing = arguments.trim();
    match named || trailing.is_empty() {
        true => filled,
        false => format!("{filled}\n\n{trailing}"),
    }
}

fn substitute(body: &str, arguments: &str, words: &[String]) -> (String, bool) {
    let mut filled = String::with_capacity(body.len());
    let mut rest = body;
    let mut named = false;

    while let Some(at) = rest.find('$') {
        filled.push_str(&rest[..at]);
        let placeholder = &rest[at..];

        if let Some(tail) = placeholder.strip_prefix(ARGUMENTS) {
            filled.push_str(arguments.trim());
            named = true;
            rest = tail;
            continue;
        }

        match nth(placeholder) {
            Some((nth, tail)) => {
                filled.push_str(words.get(nth - 1).map_or("", String::as_str));
                named = true;
                rest = tail;
            }
            None => {
                filled.push('$');
                rest = &placeholder[1..];
            }
        }
    }

    filled.push_str(rest);
    (filled, named)
}

fn nth(placeholder: &str) -> Option<(usize, &str)> {
    let digit = placeholder[1..].chars().next()?.to_digit(10)?;
    let nth = usize::try_from(digit).ok()?;
    (nth > 0).then(|| (nth, &placeholder[2..]))
}
