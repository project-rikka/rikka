use browser_pages::registrable_site;

use browser_model::Home;

pub(crate) fn crosses(from: &str, to: &str) -> bool {
    match (registrable_site(from), registrable_site(to)) {
        (Some(from), Some(to)) => from != to,
        _ => false,
    }
}

pub(crate) fn peeks_from(home: Home) -> bool {
    matches!(home, Home::Pinned(_) | Home::Favorite(_))
}
