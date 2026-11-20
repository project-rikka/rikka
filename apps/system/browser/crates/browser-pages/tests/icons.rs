use browser_pages::{IconCandidate, best_icon, data_url_payload};

fn icon<'a>(
    url: &'a str,
    rel: &'a str,
    sizes: &'a [String],
    mime: Option<&'a str>,
) -> IconCandidate<'a> {
    IconCandidate {
        url,
        rel,
        sizes,
        mime_type: mime,
    }
}

fn sizes(listed: &[&str]) -> Vec<String> {
    listed.iter().map(|size| (*size).to_owned()).collect()
}

#[test]
fn the_icon_nearest_the_shown_size_wins() {
    let (small, exact, large, any, none) = (
        sizes(&["16x16"]),
        sizes(&["64x64"]),
        sizes(&["512x512"]),
        sizes(&["any"]),
        sizes(&[]),
    );
    let page = "https://a.example/deep/page";
    let cases: [(Vec<IconCandidate<'_>>, bool, Option<&str>); 7] = [
        (
            vec![
                icon("https://a.example/16.png", "icon", &small, None),
                icon("https://a.example/512.png", "icon", &large, None),
                icon("https://a.example/64.png", "icon", &exact, None),
            ],
            true,
            Some("https://a.example/64.png"),
        ),
        (
            vec![
                icon("https://a.example/16.png", "icon", &small, None),
                icon("https://a.example/512.png", "icon", &large, None),
            ],
            true,
            Some("https://a.example/16.png"),
        ),
        (
            vec![
                icon("https://a.example/16.png", "icon", &small, None),
                icon(
                    "https://a.example/i.svg",
                    "icon",
                    &none,
                    Some("image/svg+xml"),
                ),
                icon("https://a.example/a.png", "icon", &any, None),
            ],
            true,
            Some("https://a.example/i.svg"),
        ),
        (
            vec![
                icon(
                    "https://a.example/touch.png",
                    "apple-touch-icon",
                    &exact,
                    None,
                ),
                icon("https://a.example/16.png", "shortcut icon", &small, None),
            ],
            true,
            Some("https://a.example/16.png"),
        ),
        (
            vec![icon("blob:https://a.example/1", "icon", &exact, None)],
            true,
            Some("https://a.example/favicon.ico"),
        ),
        (Vec::new(), false, None),
        (
            vec![icon("data:image/png;base64,AAAA", "icon", &none, None)],
            false,
            Some("data:image/png;base64,AAAA"),
        ),
    ];
    for (candidates, loaded, expected) in cases {
        let listed = format!("{candidates:?}");
        assert_eq!(
            best_icon(page, candidates, loaded).as_deref(),
            expected,
            "{listed}"
        );
    }
    assert_eq!(best_icon("about:blank", Vec::new(), true), None);
}

#[test]
fn data_urls_give_their_bytes() {
    let cases: [(&str, Option<&[u8]>); 8] = [
        ("data:image/png;base64,iVBORw==", Some(b"\x89PNG")),
        ("data:image/png;BASE64,iVBORw", Some(b"\x89PNG")),
        ("data:image/png;base64,iVBO%0A Rw%3D%3D", Some(b"\x89PNG")),
        (
            "data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 fill=%22%23f00%22/%3E",
            Some(b"<svg xmlns=\"http://www.w3.org/2000/svg\" fill=\"#f00\"/>"),
        ),
        ("data:image/svg+xml;charset=utf-8,<svg/>", Some(b"<svg/>")),
        ("data:,", Some(b"")),
        ("data:image/png;base64,***", None),
        ("https://a.example/icon.png", None),
    ];
    for (url, expected) in cases {
        assert_eq!(data_url_payload(url).as_deref(), expected, "{url}");
    }
}
