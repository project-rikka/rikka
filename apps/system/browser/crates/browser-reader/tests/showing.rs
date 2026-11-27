use browser_reader::{DOCUMENT, PAGE, serve};
use futures_util::StreamExt;
use rikka_web::{Event, Runtime, SessionSettings};

use crate::support::{PATIENCE, block_on, ready};

const TITLE: &str = "Long article";

#[test]
fn articles_beyond_the_message_limit_show_whole_without_scripts() {
    let paragraphs: String = (0..40_000)
        .map(|paragraph| format!("<p>Paragraph {paragraph} — ünïcödé “text” keeps going.</p>\n"))
        .collect();
    let document = format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><script>document.title = \"injected\";</script><title>{TITLE}</title></head><body>{paragraphs}</body></html>"
    );
    assert!(document.len() > 2 << 20);

    let engine = Runtime::builder().start().expect("Chrome starts");
    block_on(async {
        ready(&engine).await;
        let view = engine
            .session(SessionSettings::new().url(engine.bundled_page(PAGE)))
            .expect("open session");
        let mut events = view.events().expect("events not yet taken");
        tokio::time::timeout(PATIENCE, async {
            while let Some(event) = events.next().await {
                match event {
                    Event::Ask(ask) if ask.name == DOCUMENT => serve(ask, &document),
                    Event::Changed if view.page().title == TITLE => return,
                    Event::Closed(reason) => panic!("session closed: {reason:?}"),
                    _ => {}
                }
            }
            panic!("page events ended before the article showed");
        })
        .await
        .expect("the article showed in time");
    });
    drop(engine);
    rikka_web::wait_for_closed_runtimes();
}
