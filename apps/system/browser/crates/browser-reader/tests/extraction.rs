use std::time::Duration;

use browser_reader::{
    Article, Block, EXTRACT_SCRIPT, Inline, Labels, Look, PROBE_SCRIPT, parse, readable, render,
};
use futures_util::StreamExt;
use rikka_picture::SharedGpu;
use rikka_test_renderer::TestRenderer;
use rikka_web::{Event, Load, Runtime, RuntimeEvent, SessionSettings, World};
use serde_json::Value;

const PATIENCE: Duration = Duration::from_secs(30);
const STARTUP: Duration = Duration::from_secs(45);
const WORLD: &str = "rikka-reader";

struct Page {
    probe: Value,
    extract: Value,
    took: Duration,
}

fn gpu() -> TestRenderer {
    let renderer = rikka_test_renderer::renderer("browser-reader-test").expect("Vulkan 1.3 device");
    SharedGpu::from_gpu(renderer.gpu()).publish();
    renderer
}

fn based(html: &str, url: &str) -> String {
    let base = format!("<base href=\"{url}\">");
    match html.find("<head>") {
        Some(at) => format!("{}{base}{}", &html[..at + 6], &html[at + 6..]),
        None => format!("{base}{html}"),
    }
}

fn read(html: &str, url: &str) -> Page {
    let _gpu = gpu();
    let engine = Runtime::builder().start().expect("start installed engine");
    let mut started = engine.events().expect("events not yet taken");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("create tokio runtime");
    let page = runtime.block_on(async {
        tokio::time::timeout(STARTUP, async {
            while let Some(event) = started.next().await {
                match event {
                    RuntimeEvent::Started { .. } => return,
                    RuntimeEvent::Exited(exit) => panic!("engine exited: {exit:?}"),
                    _ => {}
                }
            }
        })
        .await
        .expect("engine started in time");
        let view = engine
            .session(SessionSettings::new())
            .expect("open session");
        let mut events = view.events().expect("events not yet taken");
        view.load(Load::html(based(html, url)));
        let loaded = tokio::time::timeout(PATIENCE, async {
            while let Some(event) = events.next().await {
                match event {
                    Event::PageStopped { success: true } => return,
                    Event::PageStopped { success: false } => panic!("page load failed"),
                    Event::Closed(reason) => panic!("session closed: {reason:?}"),
                    Event::LoadError(failure) => {
                        panic!("page load failed: {:?}", failure.error)
                    }
                    _ => {}
                }
            }
            panic!("page events ended before load");
        })
        .await;
        loaded.expect("page loaded in time");
        let world = World::isolated(WORLD);
        let reading = std::time::Instant::now();
        let probe = view
            .evaluate(PROBE_SCRIPT, &world)
            .await
            .expect("run probe script");
        let extract = view
            .evaluate(EXTRACT_SCRIPT, &world)
            .await
            .expect("run extraction script");
        Page {
            probe,
            extract,
            took: reading.elapsed(),
        }
    });
    drop(started);
    drop(engine);
    rikka_web::wait_for_closed_runtimes();
    page
}

fn inline_text(content: &[Inline], text: &mut String) {
    for inline in content {
        match inline {
            Inline::Text(run) => text.push_str(&run.text),
            Inline::Break => text.push('\n'),
        }
    }
}

fn block_text(block: &Block, text: &mut String) {
    match block {
        Block::Heading { content, .. } | Block::Paragraph(content) => inline_text(content, text),
        Block::List(list) => list
            .items
            .iter()
            .flatten()
            .for_each(|block| block_text(block, text)),
        Block::Quote(blocks) => blocks.iter().for_each(|block| block_text(block, text)),
        Block::Code(code) => text.push_str(&code.text),
        Block::Figure(figure) => inline_text(&figure.caption, text),
        Block::Table(table) => {
            for cell in table.rows.iter().flatten() {
                inline_text(&cell.content, text);
                text.push('\t');
            }
        }
        Block::Rule => {}
    }
    text.push('\n');
}

fn text_of(article: &Article) -> String {
    let mut text = String::new();
    for block in article.blocks() {
        block_text(block, &mut text);
    }
    text
}

fn figures(article: &Article) -> Vec<&browser_reader::Figure> {
    article
        .blocks()
        .iter()
        .filter_map(|block| match block {
            Block::Figure(figure) => Some(figure),
            _ => None,
        })
        .collect()
}

#[test]
fn news_article_keeps_story() {
    let page = read(
        include_str!("fixtures/news.html"),
        "https://daily.example/2024/04/18/park",
    );
    assert!(
        readable(&page.probe).expect("valid probe result"),
        "{}",
        page.probe
    );
    let article = parse(&page.extract).unwrap_or_else(|error| panic!("{error}: {}", page.extract));
    assert_eq!(
        article.title(),
        "City council approves the new riverside park"
    );
    assert_eq!(article.byline(), Some("Jane Rivers"));
    assert_eq!(article.site(), Some("The Daily Example"));
    assert_eq!(article.published(), Some("2024-04-18T09:30:00+01:00"));
    assert_eq!(article.language(), Some("en"));

    let text = text_of(&article);
    for phrase in [
        "twelve-hectare park",
        "green space",
        "We have waited a long time",
        "What happens next",
        "Consultation opens in May",
        "affordable homes",
    ] {
        assert!(text.contains(phrase), "{phrase:?} is missing from {text}");
    }
    for dropped in [
        "Share on Social",
        "Related stories",
        "Finally!",
        "Most read",
        "newsletter",
        "Copyright",
        "cookies",
        "Sponsored",
        "screen readers",
        "hidden paragraph",
        "Weather and the long forecast",
    ] {
        assert!(!text.contains(dropped), "{dropped:?} leaked into {text}");
    }
    assert!(
        !matches!(article.blocks().first(), Some(Block::Heading { .. })),
        "the title is not repeated"
    );

    let figures = figures(&article);
    assert_eq!(figures.len(), 1);
    assert_eq!(
        figures[0].images[0].source.as_str(),
        "https://daily.example/images/park-plan-1600.jpg"
    );
    assert_eq!(figures[0].images[0].width, Some(1600));
    let mut caption = String::new();
    inline_text(&figures[0].caption, &mut caption);
    assert!(caption.starts_with("The plan includes wetlands"));

    let linked = article.blocks().iter().any(|block| match block {
        Block::Paragraph(content) => content.iter().any(|inline| {
            matches!(inline, Inline::Text(run) if run.link.as_ref().is_some_and(|link| link.as_str() == "https://daily.example/timeline"))
        }),
        _ => false,
    });
    assert!(linked, "inline links survive with absolute targets");
    assert!(
        article
            .blocks()
            .iter()
            .any(|block| matches!(block, Block::Quote(_)))
    );
    assert!(
        article.blocks().iter().any(
            |block| matches!(block, Block::List(list) if !list.ordered && list.items.len() == 2)
        )
    );
    assert!(
        article
            .blocks()
            .iter()
            .any(|block| matches!(block, Block::Heading { level: 2, .. }))
    );
    assert!(
        article.lead().is_none(),
        "an article that opens with a picture needs no social image"
    );
}

#[test]
fn portal_is_not_readable() {
    let page = read(
        include_str!("fixtures/index.html"),
        "https://portal.example/",
    );
    assert!(
        !readable(&page.probe).expect("valid probe result"),
        "{}",
        page.probe
    );
    assert!(parse(&page.extract).is_err(), "{}", page.extract);
}

#[test]
fn hostile_page_cannot_inject_markup() {
    let page = read(
        include_str!("fixtures/hostile.html"),
        "https://hostile.example/story",
    );
    let article = parse(&page.extract).unwrap_or_else(|error| panic!("{error}: {}", page.extract));
    let html = render(&article, &Look::default(), &Labels::default());
    let body = &html[html.find("<body>").expect("body element")..];
    assert!(!html.contains("<script"), "{html}");
    assert!(!body.contains("javascript:"), "{body}");
    assert!(!body.contains("<img src=x"), "{body}");
    assert!(!body.contains("\" onerror"), "{body}");
    assert!(body.contains("&lt;script&gt;alert(&quot;text&quot;)&lt;/script&gt;"));
    assert_eq!(figures(&article).len(), 1);
}

#[test]
fn large_page_extracts_in_bounded_time() {
    let links: String = (0..3_000)
        .map(|link| format!("<a href=\"/l{link}\">Navigation entry number {link}</a> "))
        .collect();
    let paragraphs: String = (0..4_000)
        .map(|paragraph| {
            format!(
                "<div><div><p>Paragraph {paragraph} explains, at some length and with care, one more part of the long story, so that the reader has plenty to read.</p></div></div>"
            )
        })
        .collect();
    let html = format!(
        "<!DOCTYPE html><html lang=\"en\"><head><title>Long read</title></head><body><nav>{links}</nav><div class=\"layout\"><article><div class=\"content\">{paragraphs}</div></article></div></body></html>"
    );
    let page = read(&html, "https://long.example/read");
    assert!(page.took < Duration::from_secs(20));
    assert!(readable(&page.probe).expect("valid probe result"));
    let article = parse(&page.extract).unwrap_or_else(|error| panic!("{error}"));
    assert!(article.blocks().len() > 1_000);
    assert!(!text_of(&article).contains("Navigation entry"));
}
