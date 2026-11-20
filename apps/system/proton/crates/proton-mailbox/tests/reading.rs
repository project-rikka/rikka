use proton_mailbox::{Body, Plain, decode};
use rikka_proton_mail::MimeType;

#[test]
fn html_drops_remote_content() {
    let body = b"<p>Look</p><img src=\"https://tracker.example/pixel.gif\">\
        <script>alert(1)</script><style>p{color:red}</style>";

    let text = decode(&MimeType::Html, body).text().to_owned();

    assert!(!text.contains("tracker.example"), "{text}");
    assert!(!text.contains("alert"), "{text}");
    assert!(!text.contains("color:red"), "{text}");
}

#[test]
fn reads_mime_html_part() {
    let raw = b"Content-Type: multipart/alternative; boundary=\"edge\"\r\n\r\n\
        --edge\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\r\n\
        the plain one\r\n\
        --edge\r\n\
        Content-Type: text/html; charset=utf-8\r\n\
        Content-Transfer-Encoding: quoted-printable\r\n\r\n\
        <p>the =\r\nrich one</p>\r\n\
        --edge--\r\n";

    let words = decode(&MimeType::MultipartMixed, raw);

    let Body::Markdown(rich) = words else {
        panic!("{words:?}");
    };
    assert_eq!(rich.source().as_ref(), "the rich one");
}

#[test]
fn decodes_mime_body_past_attachments() {
    let base64_with_a_file = b"Content-Type: multipart/mixed; boundary=\"edge\"\r\n\r\n\
        --edge\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\
        Content-Transfer-Encoding: base64\r\n\r\n\
        aGVsbG8gdGhlcmU=\r\n\
        --edge\r\n\
        Content-Type: application/pdf; name=\"bill.pdf\"\r\n\
        Content-Disposition: attachment; filename=\"bill.pdf\"\r\n\
        Content-Transfer-Encoding: base64\r\n\r\n\
        JVBERg==\r\n\
        --edge--\r\n";
    let shift_jis = b"Content-Type: text/plain; charset=shift_jis\r\n\r\n\x93\xfa\x96{\x8c\xea";

    for (raw, expected) in [
        (&base64_with_a_file[..], "hello there"),
        (&shift_jis[..], "日本語"),
    ] {
        assert_eq!(
            decode(&MimeType::MultipartMixed, raw),
            Body::Plain(Plain::new(expected))
        );
    }
}
