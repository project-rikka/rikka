use proton_mailbox::{Body, Draft, Markdown, Person, Plain, addresses};
use rikka_proton_account::AddressId;
use rikka_proton_mail::{DraftAction, MessageId};

fn opened() -> proton_mailbox::OpenedMessage {
    proton_mailbox::OpenedMessage {
        id: MessageId::from("message-1"),
        address: AddressId::from("address-1"),
        subject: "Lunch".to_owned(),
        sender: Person::new("ana@proton.me", "Ana"),
        to: vec![
            Person::new("me@proton.me", "Me"),
            Person::new("bo@proton.me", "Bo"),
        ],
        cc: vec![Person::new("cy@proton.me", "")],
        bcc: Vec::new(),
        reply_to: Vec::new(),
        time: jiff::Timestamp::from_second(1_700_000_000).unwrap(),
        unread: true,
        starred: false,
        draft: false,
        words: Body::Plain(Plain::new("one\n\ntwo")),
        signature: proton_mailbox::Verification::Verified,
        files: Vec::new(),
        labels: Vec::new(),
        external: None,
    }
}

#[test]
fn parses_typed_addresses() {
    let parsed = addresses("Ana <ana@proton.me>, bo@proton.me; \n cy@proton.me , ana@PROTON.me");

    assert_eq!(
        parsed,
        vec![
            "ana@proton.me".to_owned(),
            "bo@proton.me".to_owned(),
            "cy@proton.me".to_owned()
        ]
    );
}

#[test]
fn reply_all_excludes_sender() {
    let draft = Draft::replying(&opened(), true, None, "Ana wrote:");

    assert_eq!(draft.to, "Ana <ana@proton.me>");
    assert_eq!(
        draft.cc,
        "Me <me@proton.me>, Bo <bo@proton.me>, cy@proton.me"
    );
    assert_eq!(
        draft.answering.expect("it answers").action,
        DraftAction::ReplyAll
    );
}

#[test]
fn editing_draft_keeps_its_content() {
    let mut message = opened();
    message.draft = true;

    let draft = Draft::editing(&message);

    assert_eq!(draft.id.as_ref().map(MessageId::as_str), Some("message-1"));
    assert_eq!(draft.to, "Me <me@proton.me>, Bo <bo@proton.me>");
    assert_eq!(draft.subject, "Lunch");
    assert_eq!(draft.body, "one\n\ntwo");
    assert!(draft.answering.is_none());
}

#[test]
fn reply_quotes_rich_text_with_link_targets() {
    let mut message = opened();
    message.words = Body::Markdown(Markdown::from_html(
        "<p>Price: <b>$5</b> &amp; <a href=\"https://example.com\">more</a></p>",
    ));

    let draft = Draft::replying(&message, false, None, "Ana wrote:");

    assert_eq!(
        draft.body,
        "\n\nAna wrote:\n\n> Price: $5 & more <https://example.com>\n"
    );
}
