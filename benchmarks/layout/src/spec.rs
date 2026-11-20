#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Row,
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extent {
    pub width: f32,
    pub height: f32,
}

impl Extent {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxStyle {
    pub direction: Direction,
    pub wrap: bool,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub grow: f32,
    pub shrink: f32,
    pub basis: Option<f32>,
    pub padding: f32,
    pub margin: f32,
    pub gap: f32,
    pub align: Align,
    pub justify: Justify,
}

impl BoxStyle {
    pub const fn new(direction: Direction) -> Self {
        Self {
            direction,
            wrap: false,
            width: None,
            height: None,
            grow: 0.0,
            shrink: 0.0,
            basis: None,
            padding: 0.0,
            margin: 0.0,
            gap: 0.0,
            align: Align::Stretch,
            justify: Justify::Start,
        }
    }

    pub const fn row() -> Self {
        Self::new(Direction::Row)
    }

    pub const fn column() -> Self {
        Self::new(Direction::Column)
    }

    #[must_use]
    pub const fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    #[must_use]
    pub const fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    #[must_use]
    pub const fn flex(mut self, grow: f32, shrink: f32, basis: Option<f32>) -> Self {
        self.grow = grow;
        self.shrink = shrink;
        self.basis = basis;
        self
    }

    #[must_use]
    pub const fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub const fn margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    #[must_use]
    pub const fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    #[must_use]
    pub const fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    #[must_use]
    pub const fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    #[must_use]
    pub const fn justify(mut self, justify: Justify) -> Self {
        self.justify = justify;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub style: BoxStyle,
    pub content: Option<Extent>,
    pub children: Vec<Spec>,
}

impl Spec {
    pub fn container(style: BoxStyle, children: Vec<Spec>) -> Self {
        Self {
            style,
            content: None,
            children,
        }
    }

    pub fn leaf(style: BoxStyle, content: Extent) -> Self {
        Self {
            style,
            content: Some(content),
            children: Vec::new(),
        }
    }

    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Spec::count).sum::<usize>()
    }

    pub fn leaves(&self) -> Vec<usize> {
        let mut leaves = Vec::new();
        self.visit(&mut 0, &mut |index, spec| {
            if spec.content.is_some() {
                leaves.push(index);
            }
        });
        leaves
    }

    pub fn visit(&self, index: &mut usize, visitor: &mut impl FnMut(usize, &Spec)) {
        visitor(*index, self);
        *index += 1;
        for child in &self.children {
            child.visit(index, visitor);
        }
    }

    pub fn visit_mut(&mut self, index: &mut usize, visitor: &mut impl FnMut(usize, &mut Spec)) {
        visitor(*index, self);
        *index += 1;
        for child in &mut self.children {
            child.visit_mut(index, visitor);
        }
    }

    pub fn root_extent(&self) -> Extent {
        Extent::new(
            self.style
                .width
                .expect("benchmark root should have a width"),
            self.style
                .height
                .expect("benchmark root should have a height"),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
