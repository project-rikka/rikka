use discord_core::{AttachmentKind, ChannelId, MessageId, MessageKind, Rich, SystemEvent, UserId};
use discord_translate::{Reader, avatar, message};
use discord_wire::Message as WireMessage;

const ME: &str = "100";

fn wire(json: &str) -> WireMessage {
    serde_json::from_str(json).unwrap()
}

fn reader(roles: &[String]) -> Reader<'_> {
    Reader {
        me: ME,
        roles,
        suppress_everyone: false,
        suppress_roles: false,
    }
}

#[test]
fn gateway_message_becomes_model_message() {
    let payload = wire(
        r#"{
            "id": "175928847299117063",
            "channel_id": "20",
            "type": 19,
            "author": {"id": "7", "username": "ann", "global_name": "Ann"},
            "content": "**hi** <@100>",
            "timestamp": "2016-04-30T11:18:25.796000+00:00",
            "edited_timestamp": "2016-04-30T11:20:00+00:00",
            "mentions": [{"id": "100", "username": "me"}],
            "nonce": "175928847299117000",
            "flags": 32,
            "attachments": [
                {"id": "1", "filename": "SPOILER_cat.png", "size": 10, "url": "https://cdn/x.png",
                 "proxy_url": "https://media/x.png", "content_type": "image/png",
                 "width": 2200, "height": 700},
                {"id": "2", "filename": "voice.ogg", "size": 5, "url": "https://cdn/v.ogg",
                 "content_type": "audio/ogg", "duration_secs": 1.5}
            ],
            "reactions": [{"emoji": {"name": "👍"}, "count": 3, "me": false, "me_burst": true}],
            "message_reference": {"message_id": "15", "channel_id": "20", "type": 0},
            "referenced_message": {
                "id": "15", "channel_id": "20",
                "author": {"id": "100", "username": "me"},
                "content": "earlier", "attachments": []
            }
        }"#,
    );
    let translated = message(&payload, &reader(&[])).unwrap();
    assert_eq!(translated.id, MessageId::new(175_928_847_299_117_063));
    assert_eq!(translated.channel, ChannelId::new(20));
    assert_eq!(translated.author.id, UserId::new(7));
    assert_eq!(translated.at, 1_462_015_105_796);
    assert_eq!(translated.edited, Some(1_462_015_200_000));
    assert_eq!(translated.kind, MessageKind::Normal);
    assert_eq!(translated.content, discord_markdown::parse("**hi** <@100>"));
    assert!(translated.mentions_me);
    assert_eq!(
        translated.thread,
        Some(ChannelId::new(175_928_847_299_117_063))
    );
    assert_eq!(
        translated.nonce,
        Some(MessageId::new(175_928_847_299_117_000))
    );

    let kinds: Vec<(&AttachmentKind, bool)> = translated
        .attachments
        .iter()
        .map(|attachment| (&attachment.kind, attachment.spoiler))
        .collect();
    assert_eq!(
        kinds,
        [
            (
                &AttachmentKind::Image {
                    width: 2200,
                    height: 700
                },
                true
            ),
            (&AttachmentKind::Audio { millis: Some(1500) }, false),
        ]
    );
    assert_eq!(
        translated.attachments[0]
            .preview
            .as_ref()
            .map(|picture| picture.url()),
        Some("https://media/x.png?width=1100&height=350")
    );

    assert_eq!(translated.reactions.len(), 1);
    assert!(translated.reactions[0].me);
    let reply = translated.reply.unwrap();
    assert_eq!(reply.message, MessageId::new(15));
    let quoted = reply.quoted.unwrap();
    assert!(quoted.mentions_author);
    assert_eq!(quoted.excerpt, Rich::plain("earlier"));
}

#[test]
fn mentions_follow_suppression() {
    let roles = ["9".to_owned()];
    let cases = [
        (r#""mentions": [{"id": "100"}]"#, false, false, true),
        (r#""mention_everyone": true"#, false, false, true),
        (r#""mention_everyone": true"#, true, false, false),
        (r#""mention_roles": ["9"]"#, false, false, true),
        (r#""mention_roles": ["9"]"#, false, true, false),
        (r#""mention_roles": ["8"]"#, false, false, false),
    ];
    for (field, suppress_everyone, suppress_roles, expected) in cases {
        let payload = wire(&format!(
            r#"{{"id": "1", "channel_id": "2", "author": {{"id": "3"}}, {field}}}"#
        ));
        let reader = Reader {
            suppress_everyone,
            suppress_roles,
            ..reader(&roles)
        };
        assert_eq!(reader.mentioned(&payload), expected, "{field}");
    }
}

#[test]
fn forwarded_message_shows_its_snapshot() {
    let payload = wire(
        r#"{
            "id": "1", "channel_id": "2", "author": {"id": "3"}, "content": "",
            "message_reference": {"message_id": "9", "channel_id": "8", "type": 1},
            "message_snapshots": [{"message": {"content": "original", "attachments": []}}]
        }"#,
    );
    let translated = message(&payload, &reader(&[])).unwrap();
    assert_eq!(translated.text, "original");
    assert!(translated.reply.is_none());
}

#[test]
fn system_messages_carry_events() {
    let cases = [
        (7, "", SystemEvent::Joined),
        (6, "", SystemEvent::Pinned),
        (4, "new name", SystemEvent::Renamed("new name".to_owned())),
        (99, "", SystemEvent::Other("unknown".to_owned())),
    ];
    for (kind, content, expected) in cases {
        let payload = wire(&format!(
            r#"{{"id": "1", "channel_id": "2", "author": {{"id": "3"}}, "type": {kind}, "content": "{content}"}}"#
        ));
        let translated = message(&payload, &reader(&[])).unwrap();
        assert_eq!(translated.kind, MessageKind::System(expected), "{kind}");
        assert!(translated.content.is_empty());
    }
    let anonymous = wire(r#"{"id": "1", "channel_id": "2"}"#);
    assert!(message(&anonymous, &reader(&[])).is_none());
}

#[test]
fn avatars_follow_cdn_rules() {
    let cases = [
        (
            "80351110224678912",
            Some("8342729096ea3675442027381ff50dfe"),
            None,
            "https://cdn.discordapp.com/avatars/80351110224678912/8342729096ea3675442027381ff50dfe.png",
        ),
        (
            "80351110224678912",
            Some("a_8342729096ea3675442027381ff50dfe"),
            None,
            "https://cdn.discordapp.com/avatars/80351110224678912/a_8342729096ea3675442027381ff50dfe.gif",
        ),
        (
            "80351110224678912",
            None,
            Some("1337"),
            "https://cdn.discordapp.com/embed/avatars/2.png",
        ),
        (
            "80351110224678912",
            None,
            Some("0"),
            "https://cdn.discordapp.com/embed/avatars/5.png",
        ),
    ];
    for (user, hash, discriminator, url) in cases {
        assert_eq!(avatar(user, hash, discriminator).url(), url, "{hash:?}");
    }
}
