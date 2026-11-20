use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(400.0, 300.0);
const BLOCKS: usize = 200;
const BLOCK: f32 = 60.0;
const SETTLED: Duration = Duration::from_secs(5);

#[derive(State)]
struct Page {
    built: Rc<Cell<usize>>,
}

fn view(page: &Page) -> Element {
    let blocks: Vec<Element> = (0..BLOCKS)
        .map(|at| {
            let built = Rc::clone(&page.built);
            deferred(at, BLOCK, move || {
                built.set(built.get() + 1);
                container(text(format!("block {at}")))
                    .height(Length::px(BLOCK))
                    .fill_width()
                    .build()
            })
            .build()
        })
        .collect();
    scrollable(column_of(blocks).fill_width()).grow(1.0).build()
}

#[test]
fn unreached_block_is_not_built() {
    let built = Rc::new(Cell::new(0));
    let mut scene = Scene::new();
    let mut ui = TypedUi::new(
        Page {
            built: Rc::clone(&built),
        },
        view,
        Theme::dark(),
    );
    ui.set_viewport(VIEW, 1.0);
    let now = Instant::now();
    for step in 0..8 {
        ui.render(Rendering::new(&mut scene).with_time(now + SETTLED * step));
    }

    assert!(built.get() > 0, "nothing built");
    assert!(
        built.get() < BLOCKS,
        "all {BLOCKS} blocks built for five visible: {}",
        built.get()
    );
}
