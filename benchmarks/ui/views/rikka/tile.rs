use rikka_ui::{
    Align, Behavior, Color, CornerRadii, Element, Justify, Layout, PaintCtx, Quad, Style, Styled,
    View, Widget,
};

pub struct Tile {
    child: Option<Element>,
    color: Color,
    hovered: Option<Color>,
    radius: f32,
    centered: bool,
    style: Style,
}

pub fn tile(child: impl View, color: Color) -> Tile {
    Tile {
        child: Some(child.build()),
        color,
        hovered: None,
        radius: 0.0,
        centered: false,
        style: Style::default(),
    }
}

impl Tile {
    #[must_use]
    pub fn hovered(mut self, color: Color) -> Tile {
        self.hovered = Some(color);
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Tile {
        self.radius = radius;
        self
    }

    #[must_use]
    pub fn centered(mut self) -> Tile {
        self.centered = true;
        self
    }
}

impl Widget for Tile {
    type State = ();

    fn layout(&self) -> Layout {
        let mut layout = match self.centered {
            true => Layout {
                justify: Some(Justify::Center),
                align_items: Some(Align::Center),
                ..Layout::DEFAULT
            },
            false => Layout {
                align_items: Some(Align::Center),
                ..Layout::DEFAULT
            },
        };
        self.style.apply(&mut layout);
        layout
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let color = match self.hovered {
            Some(hovered) if ctx.hovered() => hovered,
            _ => self.color,
        };
        ctx.draw_quad(
            Quad::new(ctx.bounds())
                .fill(color)
                .radius(CornerRadii::uniform(self.radius)),
        );
    }

    fn behavior(&self, (): &()) -> Behavior {
        Behavior::TRACKS_HOVER.when(self.hovered.is_some())
    }

    fn same(&self, other: &Self) -> bool {
        self.color == other.color
            && self.hovered == other.hovered
            && self.radius == other.radius
            && self.centered == other.centered
            && self.style == other.style
    }
}

impl std::fmt::Debug for Tile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Tile").finish_non_exhaustive()
    }
}

impl View for Tile {
    fn build(mut self) -> Element {
        let children = self.child.take().into_iter().collect();
        Element::with_children(self, children)
    }
}

impl Styled for Tile {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
