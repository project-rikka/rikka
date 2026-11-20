use rikka_net::Url;

pub(crate) fn inserted(base: &Url, name: &str) -> Url {
    located(base, &format!("/.well-known/{name}{}", path(base)))
}

pub(crate) fn appended(base: &Url, name: &str) -> Url {
    located(base, &format!("{}/.well-known/{name}", path(base)))
}

pub(crate) fn rooted(base: &Url, name: &str) -> Url {
    located(base, &format!("/.well-known/{name}"))
}

pub(crate) fn candidates(locations: impl IntoIterator<Item = Url>) -> Vec<Url> {
    let mut candidates = Vec::new();
    for location in locations {
        if !candidates.contains(&location) {
            candidates.push(location);
        }
    }
    candidates
}

fn path(base: &Url) -> &str {
    base.path().trim_end_matches('/')
}

fn located(base: &Url, path: &str) -> Url {
    let mut location = base.clone();
    location.set_query(None);
    location.set_fragment(None);
    location.set_path(path);
    location
}
