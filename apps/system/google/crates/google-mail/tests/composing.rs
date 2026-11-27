use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use google_mail::{Address, Enclosure, Error, Outgoing};

fn outgoing() -> Outgoing {
    Outgoing {
        from: Address::new("六花", "rikka@example.com"),
        to: vec![Address::new("Sato, Taro", "taro@example.com")],
        cc: vec![Address::new("", "cc@example.com")],
        subject: "週末の予定について — とても長い件名はエンコード語を折り返して送られます"
            .to_owned(),
        text: "こんにちは。\n二行目です。".to_owned(),
        ..Outgoing::default()
    }
}

fn parts(message: &[u8]) -> (String, String) {
    let message = String::from_utf8(message.to_vec()).expect("ascii mail");
    let (head, body) = message.split_once("\r\n\r\n").expect("a head and a body");
    (head.to_owned(), body.to_owned())
}

fn unwrapped(body: &str) -> Vec<u8> {
    STANDARD
        .decode(body.replace("\r\n", ""))
        .expect("base64 content")
}

#[test]
fn plain_message_is_seven_bit_clean_mime() {
    let message = outgoing().to_rfc822().expect("a message");

    assert!(message.is_ascii());
    let (head, body) = parts(&message);
    assert!(head.contains("From: =?UTF-8?B?5YWt6Iqx?= <rikka@example.com>\r\n"));
    assert!(head.contains("To: \"Sato, Taro\" <taro@example.com>\r\n"));
    assert!(head.contains("Cc: cc@example.com\r\n"));
    assert!(head.contains("MIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8"));
    assert!(head.contains("\r\nDate: "));
    assert!(!head.contains("Bcc") && !head.contains("In-Reply-To"));
    assert!(head.lines().all(|line| line.len() <= 78), "{head}");
    assert_eq!(
        String::from_utf8(unwrapped(&body)).expect("utf-8"),
        "こんにちは。\r\n二行目です。"
    );
    assert!(body.lines().all(|line| line.len() <= 76));
}

#[test]
fn reply_with_files_threads_and_encloses() {
    let mut reply = outgoing();
    reply.in_reply_to = Some("<parent@example.com>".to_owned());
    reply.references = vec![
        "<root@example.com>".to_owned(),
        "<parent@example.com>".to_owned(),
    ];
    reply.enclosures = vec![
        Enclosure {
            name: "report.pdf".to_owned(),
            mime: "application/pdf".to_owned(),
            content: vec![0, 159, 146, 150],
        },
        Enclosure {
            name: "資料 1.txt".to_owned(),
            mime: String::new(),
            content: b"notes".to_vec(),
        },
    ];

    let (head, body) = parts(&reply.to_rfc822().expect("a message"));

    assert!(head.contains("In-Reply-To: <parent@example.com>\r\n"));
    assert!(head.contains("References: <root@example.com>\r\n <parent@example.com>\r\n"));
    let boundary = head
        .split("boundary=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("a boundary");
    let sections: Vec<&str> = body.split(&format!("--{boundary}")).collect();
    assert_eq!(sections.len(), 5);
    assert_eq!(sections[4].trim(), "--");
    assert!(sections[2].contains("Content-Type: application/pdf\r\n"));
    assert!(sections[2].contains("Content-Disposition: attachment; filename=\"report.pdf\""));
    let pdf = sections[2].split("\r\n\r\n").nth(1).expect("content");
    assert_eq!(unwrapped(pdf), [0, 159, 146, 150]);
    assert!(sections[3].contains("Content-Type: application/octet-stream\r\n"));
    assert!(sections[3].contains("filename*=UTF-8''%E8%B3%87%E6%96%99%201.txt"));
}

#[test]
fn undeliverable_messages_are_refused() {
    let mut nobody = outgoing();
    nobody.to.clear();
    nobody.cc.clear();
    let mut wrong = outgoing();
    wrong.bcc = vec![Address::new("", "not an address")];

    assert!(matches!(nobody.to_rfc822(), Err(Error::NoRecipient)));
    assert!(
        matches!(wrong.to_rfc822(), Err(Error::Address(address)) if address == "not an address")
    );
}
