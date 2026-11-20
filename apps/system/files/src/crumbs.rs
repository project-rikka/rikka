use files_core::Location;
use rikka_app::prelude::{Event as Input, *};
use rikka_ui::{Element, Handler, IntoHandler, View};

const HEIGHT: f32 = 30.0;
const LINE: f32 = 17.0;
const LABEL: f32 = 13.0;
const INSET: f32 = 8.0;
const SEPARATOR: f32 = 16.0;
const CHEVRON: f32 = 15.0;
const WIDEST: f32 = 220.0;
const ELLIPSIS: &str = "…";

#[derive(Clone, Copy, Debug)]
struct Hitbox {
    rect: Rect,
    step: Option<usize>,
}

#[derive(Debug)]
struct Chip {
    hitbox: Hitbox,
    label: FittedLine,
}

#[derive(Debug, Default)]
pub(crate) struct State {
    hitboxes: Vec<Hitbox>,
    hovered: Option<usize>,
    cursor: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct Crumbs {
    steps: Vec<Location>,
    labels: Vec<String>,
    on_go: Option<Handler<(Location,)>>,
    on_edit: Option<Handler>,
}

impl Crumbs {
    pub(crate) fn new(at: &Location) -> Self {
        let steps = at.ancestry();
        let labels = steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                if index == 0 && step.is_local() {
                    "/".to_owned()
                } else {
                    step.name()
                }
            })
            .collect();
        Self {
            steps,
            labels,
            on_go: None,
            on_edit: None,
        }
    }

    #[must_use]
    pub(crate) fn on_go<M>(mut self, on_go: impl IntoHandler<(Location,), (), M>) -> Self {
        self.on_go = Some(on_go.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn on_edit<M>(mut self, on_edit: impl IntoHandler<(), (), M>) -> Self {
        self.on_edit = Some(on_edit.into_handler());
        self
    }

    fn chips(&self, bounds: Rect, template: &TextRun) -> Vec<Chip> {
        let measure = |text: &str| measure_text(&template.clone().content(text), None).width;
        let count = self.labels.len();
        let room = bounds.size.width;
        if count == 0 || room <= 0.0 {
            return Vec::new();
        }

        let mut widths: Vec<f32> = self
            .labels
            .iter()
            .map(|label| measure(label) + INSET * 2.0)
            .collect();
        let fold = measure(ELLIPSIS) + INSET * 2.0;
        let mut first = 0;
        let mut show_fold = false;
        if total_width(&widths) > room {
            for width in &mut widths {
                *width = width.min(WIDEST);
            }
            while first + 1 < count && fold_lead(first, fold) + total_width(&widths[first..]) > room
            {
                first += 1;
            }
            show_fold = first > 0;
            if show_fold && room - fold - SEPARATOR < INSET * 2.0 {
                show_fold = false;
                first = count - 1;
            }
            let lead = if show_fold { fold + SEPARATOR } else { 0.0 };
            let over = lead + total_width(&widths[first..]) - room;
            if over > 0.0 {
                widths[count - 1] = (widths[count - 1] - over).max(0.0);
            }
        }

        let top = bounds.origin.y + (bounds.size.height - HEIGHT) / 2.0;
        let mut x = bounds.origin.x;
        let mut chips = Vec::with_capacity(count - first + 1);
        if show_fold {
            chips.push(Chip {
                hitbox: Hitbox {
                    rect: Rect::new(x, top, fold, HEIGHT),
                    step: None,
                },
                label: FittedLine {
                    text: ELLIPSIS.to_owned(),
                    width: fold - INSET * 2.0,
                },
            });
            x += fold + SEPARATOR;
        }
        for (step, (name, allotted)) in self.labels.iter().zip(&widths).enumerate().skip(first) {
            let label = fitted_line(
                &template.clone().content(name.as_str()),
                allotted - INSET * 2.0,
            );
            let width = label.width + INSET * 2.0;
            chips.push(Chip {
                hitbox: Hitbox {
                    rect: Rect::new(x, top, width, HEIGHT),
                    step: Some(step),
                },
                label,
            });
            x += width + SEPARATOR;
        }
        chips
    }

    fn target(&self, hitbox: Hitbox) -> Option<&Location> {
        hitbox.step.and_then(|step| self.steps.get(step))
    }

    fn pick(&self, hitbox: Hitbox, ctx: &mut EventCtx<'_>) {
        match self.target(hitbox) {
            Some(at) => {
                if let Some(on_go) = &self.on_go {
                    ctx.call(on_go, (at.clone(),));
                }
            }
            None => self.edit(ctx),
        }
    }

    fn edit(&self, ctx: &mut EventCtx<'_>) {
        if let Some(on_edit) = &self.on_edit {
            ctx.call(on_edit, ());
        }
    }

    fn typed(&self, state: &mut State, key: Key, ctx: &mut EventCtx<'_>) {
        let Some(last) = state.hitboxes.len().checked_sub(1) else {
            return;
        };
        let moved = match key {
            Key::ArrowLeft => state.cursor.map_or(last, |index| index.saturating_sub(1)),
            Key::ArrowRight => state.cursor.map_or(0, |index| (index + 1).min(last)),
            Key::Home => 0,
            Key::End => last,
            Key::Enter | Key::Space => {
                if let Some(index) = state.cursor {
                    ctx.set_handled();
                    self.pick(state.hitboxes[index], ctx);
                }
                return;
            }
            _ => return,
        };
        ctx.set_handled();
        state.cursor = Some(moved);
        ctx.request_paint();
    }
}

impl Widget for Crumbs {
    type State = State;

    fn measure(&self) -> Option<MeasureSpec> {
        Some(MeasureSpec::Fixed(Size::new(0.0, HEIGHT)))
    }

    fn paint(&self, state: &mut State, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let template = TextRun::new("", LABEL).no_wrap();
        let chips = self.chips(ctx.bounds(), &template);

        let home = Point::ZERO - ctx.bounds().origin;
        state.hitboxes = chips
            .iter()
            .map(|chip| Hitbox {
                rect: chip.hitbox.rect.translate(home),
                ..chip.hitbox
            })
            .collect();
        state.hovered = state.hovered.filter(|index| *index < chips.len());
        state.cursor = state.cursor.filter(|index| *index < chips.len());

        for (index, chip) in chips.iter().enumerate() {
            let rect = chip.hitbox.rect;
            if state.hovered == Some(index) {
                ctx.draw_quad(
                    Quad::new(rect)
                        .fill(theme().hover_layer())
                        .radius(Corner::Item),
                );
            }
            if state.cursor == Some(index) && ctx.focused() {
                ctx.draw_quad(
                    Quad::new(rect)
                        .fill(Color::TRANSPARENT)
                        .stroke(1.0, palette.accent)
                        .radius(Corner::Item),
                );
            }
            ctx.draw_text(
                Rect::new(
                    rect.origin.x + INSET,
                    rect.origin.y + (HEIGHT - LINE) / 2.0,
                    chip.label.width + 2.0,
                    LINE,
                ),
                TextRun::new(chip.label.text.as_str(), LABEL),
                palette.text,
            );
            if index + 1 < chips.len() {
                ctx.draw_icon(
                    Rect::new(rect.max_x(), rect.origin.y, SEPARATOR, HEIGHT),
                    IconGlyph::from(MaterialSymbol::CHEVRON_RIGHT).spec(CHEVRON),
                    palette.text_muted,
                );
            }
        }
    }

    fn event(&self, state: &mut State, event: &Input, ctx: &mut EventCtx<'_>) {
        match event {
            Input::PointerMoved(position) => {
                let hovered = hit(&state.hitboxes, *position - ctx.bounds().origin);
                if state.hovered != hovered {
                    state.hovered = hovered;
                    ctx.request_paint();
                }
            }
            Input::PointerExited | Input::PointerLeft => {
                if state.hovered.take().is_some() {
                    ctx.request_paint();
                }
            }
            Input::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                ctx.set_handled();
                let Some(index) = hit(&state.hitboxes, *position - ctx.bounds().origin) else {
                    self.edit(ctx);
                    return;
                };
                state.cursor = Some(index);
                let hitbox = state.hitboxes[index];
                if self.target(hitbox).is_some() {
                    ctx.request_focus();
                }
                self.pick(hitbox, ctx);
            }
            Input::KeyPressed { key, .. } if ctx.focused() => self.typed(state, *key, ctx),
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE
    }
}

impl View for Crumbs {
    fn build(self) -> Element {
        Element::new(self)
    }
}

fn hit(hitboxes: &[Hitbox], at: Point) -> Option<usize> {
    hitboxes.iter().position(|hitbox| hitbox.rect.contains(at))
}

fn total_width(widths: &[f32]) -> f32 {
    widths.iter().sum::<f32>() + SEPARATOR * widths.len().saturating_sub(1) as f32
}

fn fold_lead(first: usize, fold: f32) -> f32 {
    if first == 0 { 0.0 } else { fold + SEPARATOR }
}
