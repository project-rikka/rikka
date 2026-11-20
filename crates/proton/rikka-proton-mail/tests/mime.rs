use rikka_proton_mail::{Disposition, MimeMessage, html_to_text};

const ALTERNATIVE: &str = "From: =?UTF-8?B?w4lsaXNl?= <elise@example.test>\r\n\
To: \"Doe, John\" <john@example.test>, team: ana@example.test, (Bo) bo@example.test;\r\n\
Subject: =?ISO-8859-1?Q?Caf=E9_?=\r\n =?ISO-8859-1?Q?ouvert?=\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"alt\"\r\n\
\r\n\
This is a preamble.\r\n\
--alt\r\n\
Content-Type: text/plain; charset=iso-8859-1\r\n\
Content-Transfer-Encoding: quoted-printable\r\n\
\r\n\
Le caf=E9 est ouvert, venez =\r\n\
vite.\r\n\
--alt\r\n\
Content-Type: text/html; charset=utf-8\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
PHA+TGUgY2Fmw6kgZXN0IDxiPm91dmVydDwvYj48L3A+\r\n\
--alt--\r\n\
epilogue\r\n";

const RELATED: &str = "Content-Type: multipart/mixed; boundary=outer\n\
\n\
--outer\n\
Content-Type: multipart/related; boundary=inner; type=\"text/html\"\n\
\n\
--inner\n\
Content-Type: text/html\n\
\n\
<p>logo: <img src=\"cid:logo@here\"></p>\n\
--inner\n\
Content-Type: image/png\n\
Content-ID: <logo@here>\n\
Content-Transfer-Encoding: base64\n\
\n\
iVBORw0K\n\
--inner--\n\
--outer\n\
Content-Type: application/pdf\n\
Content-Disposition: attachment;\n\
\x20filename*0*=utf-8''r%C3%A9sum;\n\
\x20filename*1=\"e final.pdf\"\n\
Content-Transfer-Encoding: base64\n\
\n\
JVBERi0=\n\
--outer\n\
Content-Type: message/rfc822\n\
\n\
Subject: Forwarded note\n\
From: someone@example.test\n\
\n\
the forwarded body\n\
--outer--\n";

const PROTECTED: &str = "Content-Type: multipart/mixed; boundary=\"p\"; protected-headers=\"v1\"\r\n\
Subject: Hidden subject\r\n\
From: ana@example.test\r\n\
\r\n\
--p\r\n\
Content-Type: text/rfc822-headers; protected-headers=\"v1\"\r\n\
Content-Disposition: inline\r\n\
\r\n\
Subject: Hidden subject\r\n\
--p\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
the real body\r\n\
--p--\r\n";

const LEGACY: &str = "Content-Type: multipart/mixed; boundary=\"q\"\r\n\
\r\n\
--q\r\n\
Content-Type: text/rfc822-headers\r\n\
\r\n\
Subject: Shown only in the legacy part\r\n\
--q\r\n\
Content-Type: text/plain\r\n\
\r\n\
body text\r\n\
--q--\r\n";

#[test]
fn alternative_message_decodes_encodings() {
    let message = MimeMessage::parse(ALTERNATIVE);
    let content = message.content();

    assert_eq!(
        message.headers().get("subject").as_deref(),
        Some("Café ouvert")
    );
    assert_eq!(content.text(), "Le café est ouvert, venez vite.");
    assert_eq!(content.html(), Some("<p>Le café est <b>ouvert</b></p>"));
    assert!(content.attachments().is_empty());

    let from = message.headers().addresses("from");
    assert_eq!(from[0].name(), "Élise");
    assert_eq!(from[0].address(), "elise@example.test");

    let to = message.headers().addresses("to");
    let spelled: Vec<(&str, &str, Option<&str>)> = to
        .iter()
        .map(|address| (address.address(), address.name(), address.group()))
        .collect();
    assert_eq!(
        spelled,
        vec![
            ("john@example.test", "Doe, John", None),
            ("ana@example.test", "", Some("team")),
            ("bo@example.test", "Bo", Some("team")),
        ]
    );
}

#[test]
fn classifies_inline_images_and_attachments() {
    let content = MimeMessage::parse(RELATED).content();

    assert_eq!(
        content.html(),
        Some("<p>logo: <img src=\"cid:logo@here\"></p>")
    );

    let logo = content.inline_file("cid:logo@here").unwrap();
    assert_eq!(logo.mime_type(), "image/png");
    assert_eq!(logo.bytes(), &[0x89, b'P', b'N', b'G', b'\r', b'\n']);

    let names: Vec<&str> = content
        .attachments()
        .iter()
        .map(|file| file.name())
        .collect();
    assert_eq!(names, vec!["résume final.pdf", "Forwarded note.eml"]);

    let pdf = &content.attachments()[0];
    assert_eq!(pdf.disposition(), &Disposition::Attachment);
    assert_eq!(pdf.bytes(), b"%PDF-");

    let forwarded = MimeMessage::parse(content.attachments()[1].bytes());
    assert_eq!(forwarded.content().text(), "the forwarded body");
}

#[test]
fn reads_protected_headers() {
    let cases = [
        (PROTECTED, "Hidden subject", "the real body"),
        (LEGACY, "Shown only in the legacy part", "body text"),
    ];

    for (raw, subject, body) in cases {
        let content = MimeMessage::parse(raw).content();
        assert_eq!(content.subject().as_deref(), Some(subject));
        assert_eq!(content.text(), body);
        assert!(content.attachments().is_empty());
    }
}

#[test]
fn bare_body_stays_readable() {
    let cases = [
        ("\r\nonly a body", None, "only a body"),
        (
            "Content-Type: multipart/mixed; boundary=gone\r\n\r\nno delimiter here",
            None,
            "no delimiter here",
        ),
        (
            "Subject: hi\r\nnot a header line\r\nmore",
            Some("hi"),
            "not a header line\nmore",
        ),
    ];

    for (raw, subject, text) in cases {
        let message = MimeMessage::parse(raw);
        assert_eq!(message.headers().get("subject").as_deref(), subject);
        assert_eq!(message.content().text(), text);
        assert!(message.content().attachments().is_empty());
    }
}

#[test]
fn html_converts_to_search_text() {
    let cases = [
        ("<div>Hello,&nbsp;World!</div>", "Hello,\u{a0}World!"),
        ("<p>one</p><p>two</p>", "one\ntwo"),
        ("a<br>b<br/>c", "a\nb\nc"),
        (
            "<style>p { color: red }</style><script>alert(1)</script>shown",
            "shown",
        ),
        (
            "<div style=\"display: none\">hidden <b>bold</b></div>kept",
            "kept",
        ),
        (
            "<!-- note -->&lt;tag&gt; &amp; &#x41;&#66; &eacute;",
            "<tag> & AB é",
        ),
        ("H<sub>2</sub>O   and \t tabs", "H2O and tabs"),
    ];

    for (html, text) in cases {
        assert_eq!(html_to_text(html), text, "{html}");
    }
}
