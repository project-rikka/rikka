use browser_pages::registrable_site;
use rikka_web::LoadRequest;

use crate::link::{self, Opening};
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

pub(crate) fn wants_peek(home: Home, from: &str, request: &LoadRequest) -> bool {
    peeks_from(home)
        && request.user_gesture
        && !request.target.opens_new_session()
        && link::opening(request.target) == Opening::Here
        && crosses(from, &request.uri)
}
