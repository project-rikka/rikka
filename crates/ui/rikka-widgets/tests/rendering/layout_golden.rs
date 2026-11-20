use crate::golden;

use std::time::{Duration, Instant};

use rikka_markdown::markdown;
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::*;
use rikka_widgets::{column, row, *};

const VIEW: Size = Size::new(640.0, 480.0);

const SETTLED: Duration = Duration::from_secs(2);

const BLESS: &str = "RIKKA_BLESS_GOLDEN";

const PARAGRAPH: &str = "A layout is only as good as the rules it keeps: text wraps inside the \
room it is given, labels that must stay on one line give way with an ellipsis, and rows that \
grow share what is left between them.";

const DOCUMENT: &str = "# Heading\n\nSome *emphasis* and `code` in a paragraph long enough to \
wrap across the width of the window.\n\n- one\n- two\n- three\n\n```rust\nfn main() {\n    \
println!(\"hello\");\n}\n```\n\n> A quote that is also long enough to need a second line when \
it is drawn.";

#[derive(State)]
struct Desk {
    name: TextDocument,
    notes: TextDocument,
    draft: TextDocument,
}

impl Desk {
    fn new() -> Self {
        Self {
            name: TextDocument::new("Rikka"),
            notes: TextDocument::new(
                "first line\nsecond line that is rather longer than the first one\nthird",
            ),
            draft: TextDocument::new("a reply\nwith two lines"),
        }
    }
}

type Viewing = fn(&Desk) -> Element;

const VIEWS: &[(&str, Viewing)] = &[
    ("controls", controls),
    ("text", texts),
    ("eliding", eliding),
    ("lists", lists),
    ("collections", collections),
    ("tiles", tiles),
    ("scrolling", scrolling),
    ("settings", settings),
    ("menu", menu_below),
    ("panes", panes),
    ("document", document),
    ("editors", editors),
    ("blocks", blocks),
    ("fitting", fitting),
];

fn controls(desk: &Desk) -> Element {
    column! {
        row! {
            button(text("Primary")).on_press(|| {});
            button(text("Ghost")).ghost().on_press(|| {});
            button(icon(MaterialSymbol::ADD)).icon_only().on_press(|| {});
            spacer().grow(1.0);
            spinner();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        row! {
            checkbox(true);
            radio(false);
            switch(true);
            text("Switch it").grow(1.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        slider(0.4).fill_width();
        progress(0.6).fill_width();
        segmented(1).segment("Day").segment("Week").segment("Month");
        dropdown(Some(0)).option("First").option("Second");
        text_field(&desk.name).fill_width();
    }
    .gap(Space::NORMAL)
    .padding(Space::ROOMY)
    .fill()
    .build()
}

fn texts(_: &Desk) -> Element {
    column! {
        text("A title").title();
        text(PARAGRAPH);
        row! {
            text(PARAGRAPH).no_wrap().grow(1.0);
            text("tail").muted();
        }
        .gap(Space::SNUG)
        .width(Length::px(300.0));
        column! {
            text(PARAGRAPH).align(TextAlign::Center);
            text("centred").caption();
        }
        .align_items(Align::Center)
        .width(Length::px(260.0));
        rich_text(PARAGRAPH);
    }
    .gap(Space::SNUG)
    .padding(Space::NORMAL)
    .fill()
    .build()
}

fn eliding(_: &Desk) -> Element {
    let chip = |widest: f32| {
        container(
            row! {
                icon(MaterialSymbol::HUB).extent(15.0);
                text("Review screenshot editor changes").text_size(12.0).no_wrap().grow(1.0);
                button(icon(MaterialSymbol::STOP_CIRCLE).extent(15.0))
                    .icon_only()
                    .ghost()
                    .small()
                    .on_press(|| {});
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .surface()
        .border(1.0, palette().outline)
        .padding(Inset::xy(10.0, 4.0))
        .max_width(Length::px(widest))
    };
    column! {
        row! { chip(240.0) }.fill_width();
        row! { chip(600.0) }.fill_width();
    }
    .gap(Space::SNUG)
    .fill()
    .build()
}

fn lists(_: &Desk) -> Element {
    row! {
        nav_list(1)
            .item(nav_item("Inbox"))
            .item(nav_item("Drafts"))
            .item(nav_item("Archive"))
            .width(Length::px(160.0))
            .fill_height();
        column! {
            list_of(40, |index| list_item(format!("row {index}")))
                .selected(Some(3))
                .height(Length::px(180.0))
                .fill_width();
        }
        .gap(Space::SNUG)
        .grow(1.0)
        .fill_height();
    }
    .gap(Space::SNUG)
    .fill()
    .build()
}

fn collections(_: &Desk) -> Element {
    row! {
        lazy_column(200, |slot| {
            let words = 1 + slot.index % 7;
            container(text(PARAGRAPH.split(' ').take(words * 4).collect::<Vec<_>>().join(" ")))
                .padding(Space::SNUG)
                .fill_width()
        })
        .gap(Space::HAIR)
        .grow(1.0)
        .fill_height();
        lazy_grid(3, 60, |slot| {
            container(text(format!("cell {}", slot.index)))
                .padding(Space::TIGHT)
                .fill_width()
        })
        .grow(1.0)
        .fill_height();
    }
    .gap(Space::SNUG)
    .fill()
    .build()
}

fn tiles(_: &Desk) -> Element {
    column! {
        lazy_tiles(Length::px(120.0), 80, |slot| {
            container(text(format!("tile {}", slot.index)))
                .height(Length::px(90.0))
                .fill_width()
        })
        .uniform(Length::px(90.0))
        .gap(Space::SNUG)
        .grow(1.0)
        .fill_width();
        lazy_row(30, |slot| {
            container(text(format!("chip {}", slot.index)))
                .width(Length::px(80.0))
                .padding(Space::TIGHT)
        })
        .height(Length::px(40.0))
        .fill_width();
    }
    .fill()
    .build()
}

fn scrolling(_: &Desk) -> Element {
    scrollable(column_of((0..40).map(|index| {
        row! {
            text(format!("setting {index}"));
            spacer().grow(1.0);
            text("value").muted();
        }
        .fill_width()
    })))
    .fill()
    .build()
}

fn settings(desk: &Desk) -> Element {
    page([
        section(
            "Appearance",
            card(column! {
                field("Name", text_field(&desk.name).width(Length::px(200.0)));
                field("Dark", switch(true));
                field("Scale", slider(0.5).width(Length::px(200.0)));
            }),
        )
        .build(),
        section("About", card(text(PARAGRAPH))).build(),
    ])
    .fill()
    .build()
}

fn menu_below(_: &Desk) -> Element {
    column! {
        button(text("Open")).on_press(|| {}).build().keyed("opener");
        menu()
            .items([menu_item("Copy"), menu_item("Paste"), menu_item("SplitRatios").submenu(true)])
            .build()
            .anchored(Anchor::below("opener").offset(0.0, 4.0));
    }
    .padding(Space::ROOMY)
    .fill()
    .build()
}

fn panes(_: &Desk) -> Element {
    split(
        Axis::Horizontal,
        0.3,
        pane(column! {
            tabs(0).item(tab("One")).item(tab("Two"));
            text(PARAGRAPH);
        }),
        pane(column! { text("second pane").title(); text(PARAGRAPH).muted() }),
    )
    .fill()
    .build()
}

fn document(_: &Desk) -> Element {
    scrollable(markdown(DOCUMENT).fill_width()).fill().build()
}

fn editors(desk: &Desk) -> Element {
    column! {
        text_editor(&desk.draft).fits_text(40.0..=120.0).fill_width();
        text_editor(&desk.notes).fill_width().grow(1.0);
    }
    .gap(Space::SNUG)
    .padding(Space::SNUG)
    .fill()
    .build()
}

fn blocks(_: &Desk) -> Element {
    column! {
        toolbar().group(tool_group([
            tool_item(button(text("Back")).on_press(|| {})),
            tool_item(button(text("Forward")).on_press(|| {})),
        ]));
        grid_of(3, (0..7).map(|index| container(text(format!("grid {index}"))).padding(Space::TIGHT)));
        row_of((0..9).map(|index| container(text(format!("tag {index}"))).padding(Space::TIGHT)))
            .wrap_items()
            .gap(Space::TIGHT)
            .width(Length::px(300.0));
        container(text("centred")).center().height(Length::px(80.0)).fill_width();
    }
    .gap(Space::SNUG)
    .padding(Space::SNUG)
    .fill()
    .build()
}
fn fitting(_: &Desk) -> Element {
    column! {
        text("A pane that must fit").title();
        container(scrollable(column_of(
            (0..40).map(|index| text(format!("line {index}"))),
        )))
        .padding(Space::SNUG);
        text("footer").muted();
    }
    .fill()
    .build()
}

fn dumped(view: Viewing) -> String {
    let mut ui = Ui::new(Desk::new(), view, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    let mut scene = Scene::new();
    let start = Instant::now();
    ui.render(Rendering::new(&mut scene).with_time(start));
    ui.render(Rendering::new(&mut scene).with_time(start + SETTLED));
    ui.dump()
}

fn sections(dumps: &str) -> Vec<(&str, &str)> {
    dumps
        .split("=== ")
        .filter(|section| !section.is_empty())
        .filter_map(|section| section.split_once('\n'))
        .collect()
}

#[test]
fn layouts_match_golden() {
    let dumped: Vec<(&str, String)> = VIEWS
        .iter()
        .map(|(name, view)| (*name, dumped(*view)))
        .collect();
    if let Some(path) = std::env::var_os(BLESS) {
        let all: String = dumped
            .iter()
            .map(|(name, dump)| format!("=== {name}\n{dump}"))
            .collect();
        let source = format!("pub(crate) const DUMPS: &str = r####\"{all}\"####;\n");
        std::fs::write(path, source).expect("failed to write golden file");
        return;
    }
    let frozen = sections(golden::DUMPS);
    for (name, dump) in &dumped {
        let Some((_, frozen_layout)) = frozen
            .iter()
            .find(|(frozen_layout, _)| frozen_layout == name)
        else {
            panic!("{name} missing from golden file");
        };
        if frozen_layout != dump {
            let first = frozen_layout
                .lines()
                .zip(dump.lines())
                .position(|(was, now)| was != now)
                .unwrap_or_else(|| frozen_layout.lines().count().min(dump.lines().count()));
            panic!(
                "{name} is laid out differently from line {}:\nfrozen: {:?}\nnow:    {:?}\n\n{dump}",
                first + 1,
                frozen_layout.lines().nth(first),
                dump.lines().nth(first)
            );
        }
    }
}
