use rikka_mime::MimeType;

#[test]
fn parse_drops_parameters() {
    let mime = MimeType::parse("image/PNG").expect("valid media type");
    assert_eq!(mime.as_str(), "image/png");
    assert_eq!(mime.kind(), "image");
    assert_eq!(mime.subtype(), "png");

    let text = MimeType::parse("text/plain; charset=utf-8").expect("valid media type");
    assert_eq!(text, MimeType::well_known(MimeType::TEXT));
}

#[test]
fn parse_rejects_invalid_type() {
    for broken in [
        "image",
        "/png",
        "image/",
        "im age/png",
        "../png",
        "image/.png",
    ] {
        assert_eq!(MimeType::parse(broken), None, "{broken}");
    }
}
