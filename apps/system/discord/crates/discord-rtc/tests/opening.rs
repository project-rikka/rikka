use discord_core::{ChannelId, Error, UserId, VoiceSink};
use discord_rtc::{Endpoint, Role, Transport};

fn endpoint(host: &str) -> Endpoint {
    Endpoint {
        endpoint: host.to_owned(),
        token: "secret-token".to_owned(),
        server: 1,
        channel: ChannelId::new(2),
        group: 2,
        user: UserId::new(3),
        session: "secret-session".to_owned(),
    }
}

#[test]
fn endpoint_debug_hides_secrets() {
    let shown = format!("{:?}", endpoint("voice.discord.media"));
    assert!(!shown.contains("secret"));
}

#[test]
fn invalid_endpoint_host_fails_to_open() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    for host in ["", "host/path", "host?query"] {
        let (sink, _events) = VoiceSink::new();
        let result = runtime.block_on(Transport::open(endpoint(host), Role::Voice, sink));
        assert!(matches!(result, Err(Error::Voice(_))), "{host:?}");
    }
}
