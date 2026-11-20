use discord_core::{
    ChannelId, Delivery, Draft, Emoji, Message, MessageId, MessageKind, Page, Presence, Reaction,
    ReactionChange, ReactionKind, Ready, ReplyTo, Rich, Store, Update, User, UserId,
};

const CHANNEL: ChannelId = ChannelId::new(7);

fn author(id: u64) -> User {
    User {
        id: UserId::new(id),
        handle: format!("user{id}"),
        display: None,
        avatar: None,
        bot: false,
    }
}

fn message(number: u64) -> Message {
    Message {
        id: MessageId::new(number),
        channel: CHANNEL,
        author: author(1),
        at: number as i64,
        edited: None,
        kind: MessageKind::Normal,
        text: format!("m{number}"),
        content: Rich::plain(format!("m{number}")),
        attachments: Vec::new(),
        embeds: Vec::new(),
        stickers: Vec::new(),
        reactions: Vec::new(),
        reply: None,
        mentioned: Vec::new(),
        mentions_me: false,
        pinned: false,
        thread: None,
        nonce: None,
        delivery: Delivery::Delivered,
    }
}

fn draft(nonce: u64, reply: Option<u64>) -> Draft {
    Draft {
        nonce: MessageId::new(nonce),
        text: "hello".into(),
        reply: reply.map(|message| ReplyTo {
            message: MessageId::new(message),
            mention: true,
        }),
        uploads: Vec::new(),
        stickers: Vec::new(),
        silent: false,
    }
}

fn signed_in() -> Store {
    let mut store = Store::default();
    store.apply(Update::Ready(Box::new(Ready {
        me: author(2),
        spaces: Vec::new(),
        folders: Vec::new(),
        roles: Vec::new(),
        emojis: Vec::new(),
        stickers: Vec::new(),
        channels: Vec::new(),
        users: Vec::new(),
        members: Vec::new(),
        presences: Vec::new(),
        friends: Vec::new(),
        unread: Vec::new(),
        voice: Vec::new(),
        calls: Vec::new(),
        own_presence: Presence::default(),
    })));
    store
}

fn listed(store: &Store) -> Vec<u64> {
    store
        .timeline(&CHANNEL)
        .unwrap()
        .messages()
        .iter()
        .map(|message| message.id.get())
        .collect()
}

#[test]
fn history_pages_merge_in_order() {
    let mut store = Store::default();

    store.absorb(&CHANNEL, &Page::Latest, vec![message(30), message(20)]);
    store.absorb(
        &CHANNEL,
        &Page::Before(MessageId::new(20)),
        vec![message(10)],
    );
    store.absorb(&CHANNEL, &Page::Before(MessageId::new(10)), Vec::new());

    assert_eq!(listed(&store), [10, 20, 30]);
    let timeline = store.timeline(&CHANNEL).unwrap();
    assert!(timeline.reached_oldest());
    assert!(timeline.reached_newest());
}

#[test]
fn echo_replaces_pending_message() {
    let mut store = signed_in();
    store.absorb(&CHANNEL, &Page::Latest, vec![message(10)]);

    store.pend(&CHANNEL, &draft(77, Some(10)));
    let pending = store.timeline(&CHANNEL).unwrap().messages()[1].clone();
    assert_eq!(pending.delivery, Delivery::Sending);
    assert!(pending.reply.as_ref().unwrap().quoted.is_some());

    let mut echo = message(60);
    echo.nonce = Some(MessageId::new(77));
    store.apply(Update::Message(echo));

    assert_eq!(listed(&store), [10, 60]);
}

#[test]
fn failed_send_visible_until_withdrawn() {
    let mut store = signed_in();
    store.pend(&CHANNEL, &draft(9, None));

    store.fail(&CHANNEL, &MessageId::new(9));
    assert_eq!(
        store.timeline(&CHANNEL).unwrap().messages()[0].delivery,
        Delivery::Failed
    );

    store.withdraw(&CHANNEL, &MessageId::new(9));
    assert!(store.timeline(&CHANNEL).unwrap().messages().is_empty());
}

fn react(store: &mut Store, kind: ReactionKind) {
    store.apply(Update::Reaction(ReactionChange {
        channel: CHANNEL,
        message: MessageId::new(1),
        kind,
    }));
}

fn reactions(store: &Store) -> Vec<Reaction> {
    store.timeline(&CHANNEL).unwrap().messages()[0]
        .reactions
        .clone()
}

#[test]
fn reactions_count_each_person_once() {
    let mut store = Store::default();
    store.absorb(&CHANNEL, &Page::Latest, vec![message(1)]);
    let thumbs = Emoji::unicode("👍");

    for me in [false, true, true] {
        react(
            &mut store,
            ReactionKind::Added {
                emoji: thumbs.clone(),
                me,
            },
        );
    }
    let counted = reactions(&store);
    assert_eq!(counted.len(), 1);
    assert_eq!((counted[0].count, counted[0].me), (2, true));

    for me in [true, false] {
        react(
            &mut store,
            ReactionKind::Removed {
                emoji: thumbs.clone(),
                me,
            },
        );
    }
    assert!(reactions(&store).is_empty());
}
