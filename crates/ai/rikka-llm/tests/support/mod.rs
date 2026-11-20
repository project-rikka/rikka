use rikka_llm::{Event, Model, Request, Response, Result};
use rikka_test_llm::Server;
use serde_json::Value;

pub(crate) async fn answer(model: &Model, request: &Request) -> Result<Response> {
    model.stream(request).await?.response().await
}

pub(crate) async fn events(model: &Model, request: &Request) -> Vec<Event> {
    let mut stream = model.stream(request).await.expect("stream should open");
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event.expect("frame should parse"));
    }
    events
}

pub(crate) fn finished(events: &[Event]) -> &Response {
    events
        .iter()
        .find_map(Event::as_done)
        .expect("stream should finish")
}

pub(crate) async fn sent(model: &Model, request: &Request, server: &Server) -> Value {
    let _ = answer(model, request).await;
    server.last()
}
