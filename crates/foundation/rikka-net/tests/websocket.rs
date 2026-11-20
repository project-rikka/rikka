use std::sync::atomic::Ordering;
use std::time::Duration;

use rikka_net::{Client, Message, ReadyState, RetryPolicy};

use crate::support::{spawn_dropping, spawn_echo};

#[tokio::test]
async fn sends_and_receives_messages() {
    let url = spawn_echo().await;

    let mut socket = Client::new()
        .websocket(&url)
        .connect()
        .await
        .expect("the handshake succeeds");

    assert_eq!(socket.state(), ReadyState::Open);

    socket.send_text("hello").await.expect("the text is sent");
    let echoed = socket
        .recv()
        .await
        .expect("a message comes back")
        .expect("it is not an error");
    assert_eq!(echoed, Message::text("hello"));

    socket
        .send_binary(vec![1u8, 2, 3])
        .await
        .expect("the bytes are sent");
    let echoed = socket
        .recv()
        .await
        .expect("a message comes back")
        .expect("it is not an error");
    assert_eq!(echoed.as_bytes(), &[1, 2, 3]);

    socket.close().await.expect("the socket closes");
    assert_eq!(socket.state(), ReadyState::Closed);
}

#[tokio::test]
async fn reconnect_replays_subscriptions() {
    let (url, connections) = spawn_dropping().await;

    let mut socket = Client::new()
        .websocket(&url)
        .reconnect(
            RetryPolicy::new()
                .max_retries(5)
                .initial_delay(Duration::from_millis(1))
                .jitter(0.0),
        )
        .send_on_reconnect([Message::text("resubscribe")])
        .connect()
        .await
        .expect("the handshake succeeds");

    let mut greetings = Vec::new();
    while greetings.len() < 3 {
        let Some(message) = socket.recv().await else {
            break;
        };
        if let Ok(message) = message
            && let Some(text) = message.as_text()
        {
            greetings.push(text.to_owned());
        }
    }

    assert_eq!(
        greetings,
        ["connection 1", "connection 2", "connection 3"],
        "each reconnection is a fresh connection to the server",
    );
    assert!(connections.load(Ordering::SeqCst) >= 3);
}
