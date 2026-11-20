use std::time::{Duration, Instant};

use rikka_markdown::{MarkdownStream, markdown, streamed_markdown};
use rikka_renderer::Scene;
use rikka_theme::{Theme, builtin_themes};
use rikka_types::{Modifiers, Point, PointerButton, Size};
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(520.0, 420.0);
const SETTLED: Duration = Duration::from_secs(5);
const BLOCK_AT_A_TIME: usize = 4 * 1024;

const EVERYTHING: &str = "\
---
title: front matter is not shown
---

# Heading one

Prose with **bold**, *italic*, ***both***, ~~struck through~~, `code`,
a [link](https://example.test), an autolink <https://example.test/auto>,
a footnote[^1] and 2^10^.

## Tables

| Left | Middle | Right |
|:-----|:------:|------:|
| one  | `two`  | three |
| four | five   | **six** |

## Lists

1. first
2. second
   - nested
   - [x] done
   - [ ] not done

> A quotation.
>
> > Nested.

> [!WARNING]
> Something to watch out for.

Term
: what the term means

```rust
fn main() {
    println!(\"hello\");
}
```

<div>raw html</div>

$$x^2$$

![a picture](nowhere.png)

---

[^1]: The note itself.
";

#[derive(State)]
struct Reader {
    source: &'static str,
    followed: Vec<String>,
}

fn reading(reader: &Reader) -> impl View {
    scrollable(
        markdown(reader.source)
            .on_link(|reader: &mut Reader, target: String| reader.followed.push(target))
            .selectable("read")
            .base("/tmp"),
    )
    .grow(1.0)
}

fn read(source: &'static str, theme: Theme) -> (TypedUi<Reader>, Scene) {
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(
        Reader {
            source,
            followed: Vec::new(),
        },
        reading,
        theme,
    );
    ui.set_viewport(VIEW, 1.0);
    let now = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(now));
    ui.render(Rendering::new(&mut scene).with_time(now + SETTLED));
    (ui, scene)
}

#[test]
fn constructs_paint_under_builtin_themes() {
    for spec in builtin_themes() {
        let theme = Theme::from_spec(spec);
        let (_ui, scene) = read(EVERYTHING, theme.clone());
        assert_eq!(
            scene.background(),
            theme.palette().background,
            "`{}` theme background not synced",
            spec.slug
        );
    }
}

fn click(ui: &mut TypedUi<Reader>, at: Point) {
    ui.handle_event(Event::PointerMoved(at));
    ui.handle_event(Event::PointerPressed {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
    ui.handle_event(Event::PointerReleased {
        position: at,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    });
}

#[test]
fn link_only_document_reports_target() {
    let (mut ui, mut scene) = read("[go](target://somewhere)", Theme::dark());

    click(&mut ui, Point::new(6.0, 10.0));
    ui.render(&mut scene);

    assert_eq!(ui.state().followed, vec!["target://somewhere".to_owned()]);
}

#[test]
fn empty_document_renders_empty_column() {
    let (ui, _scene) = read("", Theme::dark());
    assert!(!ui.needs_render());
}

#[derive(State)]
struct Streaming {
    markdown_stream: MarkdownStream,
}

fn streaming(streaming: &Streaming) -> impl View {
    column_of(vec![
        streamed_markdown(&streaming.markdown_stream)
            .fill_width()
            .build(),
    ])
    .fill_width()
}

fn settled(ui: &mut TypedUi<Streaming>, scene: &mut Scene, now: &mut Instant) {
    for _ in 0..240 {
        *now += Duration::from_millis(16);
        ui.render(Rendering::new(scene).with_time(*now));
        if !ui.needs_render() {
            return;
        }
    }
    panic!("stream never settled");
}

fn shows(ui: &TypedUi<Streaming>, words: &str) -> bool {
    ui.dump().contains(&format!("\"{words}\""))
}

#[test]
fn stream_grows_without_rebuild() {
    let markdown_stream = MarkdownStream::default();
    markdown_stream.follow(Some(0), "The first words");
    let mut ui = TypedUi::new(Streaming { markdown_stream }, streaming, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let mut now = Instant::now();
    settled(&mut ui, &mut scene, &mut now);
    assert!(!shows(&ui, "and a second paragraph"));

    let counted = stats::snapshot();
    ui.update(|streaming: &mut Streaming| {
        assert!(
            streaming
                .markdown_stream
                .grow(0, "The first words\n\nand a second paragraph")
        );
        request_frame();
    });
    settled(&mut ui, &mut scene, &mut now);
    let since = stats::snapshot().since(counted);

    assert_eq!(since.views, 0);
    assert!(since.builds >= 1);
    assert!(shows(&ui, "and a second paragraph"), "{}", ui.dump());
}
fn paragraphs(count: usize) -> String {
    (0..count)
        .map(|at| format!("{at:03} {}", "word ".repeat(32)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn paint_during_stream(after: usize) -> u64 {
    let shown = paragraphs(after);
    let markdown_stream = MarkdownStream::default();
    markdown_stream.follow(Some(0), &shown);
    let mut ui = TypedUi::new(Streaming { markdown_stream }, streaming, Theme::dark());
    ui.set_viewport(Size::new(VIEW.width, 100_000.0), 1.0);
    let mut scene = Scene::new();
    let mut now = Instant::now();
    settled(&mut ui, &mut scene, &mut now);

    let grown = paragraphs(after + 1);
    ui.update(|streaming: &mut Streaming| assert!(streaming.markdown_stream.grow(0, &grown)));
    now += Duration::from_millis(16);
    ui.render(Rendering::new(&mut scene).with_time(now));
    let counted = stats::snapshot();
    settled(&mut ui, &mut scene, &mut now);
    assert!(
        ui.dump().contains(&format!("\"{after:03} word")),
        "{}",
        ui.dump()
    );
    stats::snapshot().since(counted).painted
}

#[test]
fn stream_reveals_only_new_paragraph() {
    let crossing = BLOCK_AT_A_TIME / paragraphs(1).len();
    assert!(
        paragraphs(crossing).len() < BLOCK_AT_A_TIME
            && paragraphs(crossing + 1).len() > BLOCK_AT_A_TIME
    );

    assert_eq!(
        paint_during_stream(2),
        paint_during_stream(crossing),
        "shown paragraphs revealed again"
    );
}
