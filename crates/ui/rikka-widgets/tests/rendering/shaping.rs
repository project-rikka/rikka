use std::time::Instant;

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::*;
use rikka_widgets::*;

const LINES: usize = 200;
const LEADING: f32 = 20.0;
const VIEW: Size = Size::new(600.0, LINES as f32 * LEADING);

#[derive(State)]
struct Page;

fn paged(_: &Page) -> impl View {
    let lines = (0..LINES).map(|index| {
        text(format!("line {index} of this page"))
            .label()
            .line_height(LEADING)
    });
    column_of(lines).fill_width()
}

fn measured() -> Size {
    let mut scene = Scene::new();
    let mut ui = Ui::new(Page, paged, Theme::dark());
    ui.set_viewport(VIEW, 1.0);
    ui.render(Rendering::new(&mut scene).with_time(Instant::now()));
    ui.content_size()
}

#[test]
fn parallel_shaping_matches_serial() {
    let first = measured();
    let again = measured();

    assert_eq!(
        first.height,
        LINES as f32 * LEADING,
        "line spacing differs from leading"
    );
    assert!(first.width > 0.0, "lines have no width");
    assert_eq!(first, again, "page measured inconsistently");
}
