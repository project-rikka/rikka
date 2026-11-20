use discord_core::{ChannelId, MessageId, SpaceId, StreamKey, UserId};

#[test]
fn snowflake_encodes_creation_time() {
    let id: MessageId = "175928847299117063".parse().unwrap();
    assert_eq!(id.created_at(), 1_462_015_105_796);
    assert_eq!(
        MessageId::minted_at(1_462_015_105_796).created_at(),
        1_462_015_105_796
    );
}

#[test]
fn snowflakes_serialize_as_strings() {
    let id = ChannelId::new(81_384_788_765_712_384);
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"81384788765712384\"");
    assert_eq!(serde_json::from_str::<ChannelId>(&json).unwrap(), id);
    assert_eq!(
        serde_json::from_str::<ChannelId>("81384788765712384").unwrap(),
        id
    );
}

#[test]
fn stream_key_round_trips() {
    let guild = StreamKey {
        space: Some(SpaceId::new(1)),
        channel: ChannelId::new(2),
        owner: UserId::new(3),
    };
    assert_eq!(guild.to_string(), "guild:1:2:3");
    assert_eq!("guild:1:2:3".parse(), Ok(guild));
    let call = StreamKey {
        space: None,
        channel: ChannelId::new(4),
        owner: UserId::new(5),
    };
    assert_eq!(call.to_string(), "call:4:5");
    assert_eq!("call:4:5".parse(), Ok(call));
    for broken in ["test:5", "guild:a:2:3", "call:1:2:3"] {
        assert!(broken.parse::<StreamKey>().is_err(), "{broken}");
    }
}
