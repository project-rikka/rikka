use crate::document::Element;

use super::phrasing::Style;

const BOLD_WEIGHT: f32 = 600.0;
const IMPORTANT: &str = "!important";

pub(super) fn is_hidden(element: Element<'_>) -> bool {
    styled(element, Style::default()).is_none()
}

pub(super) fn styled(element: Element<'_>, mut style: Style) -> Option<Style> {
    if element.attribute("hidden").is_some() {
        return None;
    }
    for (property, value) in declarations(element) {
        let is = |name: &str| property.eq_ignore_ascii_case(name);
        let means = |word: &str| value.eq_ignore_ascii_case(word);
        if (is("display") && means("none"))
            || (is("visibility") && means("hidden"))
            || (is("mso-hide") && means("all"))
        {
            return None;
        }
        if is("font-weight") {
            if means("bold") || means("bolder") {
                style.strong = true;
            } else if means("normal") || means("lighter") {
                style.strong = false;
            } else if let Ok(weight) = value.parse::<f32>() {
                style.strong = weight >= BOLD_WEIGHT;
            }
        } else if is("font-style") {
            if means("normal") {
                style.emphasis = false;
            } else if means("italic") || starts_with(value, "oblique") {
                style.emphasis = true;
            }
        } else if (is("text-decoration") || is("text-decoration-line"))
            && value
                .as_bytes()
                .windows(b"line-through".len())
                .any(|window| window.eq_ignore_ascii_case(b"line-through"))
        {
            style.strike = true;
        }
    }
    Some(style)
}

fn declarations(element: Element<'_>) -> impl Iterator<Item = (&str, &str)> {
    element
        .attribute("style")
        .into_iter()
        .flat_map(|style| style.split(';'))
        .filter_map(|declaration| {
            let (property, value) = declaration.split_once(':')?;
            let value = value.trim();
            let value = match value
                .len()
                .checked_sub(IMPORTANT.len())
                .and_then(|at| value.get(at..).map(|tail| (at, tail)))
            {
                Some((at, tail)) if tail.eq_ignore_ascii_case(IMPORTANT) => value[..at].trim_end(),
                _ => value,
            };
            Some((property.trim(), value))
        })
}

fn starts_with(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}
