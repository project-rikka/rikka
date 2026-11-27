use browser_pages::data_url_payload;

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
