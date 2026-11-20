use crate::spec::{Align, BoxStyle, Extent, Spec};

#[derive(Clone, Copy)]
pub struct Scenario {
    pub name: &'static str,
    build: fn() -> Spec,
}

impl Scenario {
    const fn new(name: &'static str, build: fn() -> Spec) -> Self {
        Self { name, build }
    }

    pub fn spec(&self) -> Spec {
        (self.build)()
    }
}

impl std::fmt::Debug for Scenario {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Scenario")
            .field("name", &self.name)
            .finish()
    }
}

const SQUARE: Extent = Extent::new(10.0, 10.0);

fn stack_with_flex() -> Spec {
    let child = Spec::leaf(BoxStyle::column().flex(1.0, 1.0, Some(0.0)), SQUARE);
    Spec::container(BoxStyle::column().size(100.0, 100.0), vec![child; 10])
}

fn align_stretch() -> Spec {
    let child = Spec::leaf(BoxStyle::column().height(20.0), SQUARE);
    Spec::container(
        BoxStyle::column().size(100.0, 300.0).align(Align::Stretch),
        vec![child; 10],
    )
}

fn nested_flex() -> Spec {
    let flexible = BoxStyle::column().flex(1.0, 1.0, Some(0.0));
    let leaf = Spec::leaf(flexible, SQUARE);
    let branch = Spec::container(flexible, vec![leaf; 10]);
    Spec::container(BoxStyle::column().size(1000.0, 1000.0), vec![branch; 10])
}

fn huge_nested() -> Spec {
    let sized = BoxStyle::column().size(10.0, 10.0).flex(1.0, 0.0, None);
    let mut level = Spec::leaf(sized, SQUARE);
    for _ in 0..3 {
        level = Spec::container(sized, vec![level; 10]);
    }
    Spec::container(BoxStyle::column().size(1000.0, 1000.0), vec![level; 10])
}

fn list() -> Spec {
    const ROWS: usize = 10_000;
    const ROW: f32 = 24.0;
    let row = Spec::container(
        BoxStyle::row().padding(4.0).gap(8.0).align(Align::Center),
        vec![
            Spec::leaf(BoxStyle::row(), Extent::new(16.0, 16.0)),
            Spec::leaf(
                BoxStyle::row().flex(1.0, 1.0, None),
                Extent::new(120.0, 16.0),
            ),
            Spec::leaf(BoxStyle::row(), Extent::new(40.0, 16.0)),
        ],
    );
    Spec::container(
        BoxStyle::column()
            .size(800.0, ROWS as f32 * ROW)
            .align(Align::Stretch),
        vec![row; ROWS],
    )
}

fn tiles() -> Spec {
    const TILES: usize = 2_000;
    const WIDTH: f32 = 1200.0;
    const TILE: Extent = Extent::new(80.0, 60.0);
    const GAP: f32 = 8.0;
    const PADDING: f32 = 4.0;
    let per_line = ((WIDTH - 2.0 * PADDING + GAP) / (TILE.width + GAP)) as usize;
    let lines = TILES.div_ceil(per_line);
    let height = 2.0 * PADDING + lines as f32 * TILE.height + (lines - 1) as f32 * GAP;
    let tile = Spec::leaf(BoxStyle::row(), TILE);
    Spec::container(
        BoxStyle::row()
            .size(WIDTH, height)
            .padding(PADDING)
            .gap(GAP)
            .wrap()
            .align(Align::Start),
        vec![tile; TILES],
    )
}

fn deep() -> Spec {
    let mut nested = Spec::leaf(BoxStyle::row(), SQUARE);
    for _ in 0..200 {
        nested = Spec::container(
            BoxStyle::row().padding(1.0).align(Align::Start),
            vec![nested],
        );
    }
    Spec::container(
        BoxStyle::row().size(1000.0, 1000.0).align(Align::Start),
        vec![nested],
    )
}

pub const STACK_WITH_FLEX: Scenario = Scenario::new("stack_with_flex", stack_with_flex);
pub const ALIGN_STRETCH: Scenario = Scenario::new("align_stretch", align_stretch);
pub const NESTED_FLEX: Scenario = Scenario::new("nested_flex", nested_flex);
pub const HUGE_NESTED: Scenario = Scenario::new("huge_nested", huge_nested);
pub const LIST: Scenario = Scenario::new("list", list);
pub const TILES: Scenario = Scenario::new("tiles", tiles);
pub const DEEP: Scenario = Scenario::new("deep", deep);

pub const SCENARIOS: &[Scenario] = &[
    STACK_WITH_FLEX,
    ALIGN_STRETCH,
    NESTED_FLEX,
    HUGE_NESTED,
    LIST,
    TILES,
    DEEP,
];
