use paint_assistant::{
    BrushOrder, DrawKind, DrawOrder, GradientOrder, MarkOrder, Placing, Sample, TextOrder,
};
use paint_document::{AddMethod, Edit, Figure, FigureEdit, FigureId, LayerId};
use rikka_figure::{Affine, Form, Lettering, Pen};
use rikka_image_edit::{
    Anchor, BrushStroke, Gradient, GradientInk, Ink, Mark, Marking, Marks, Mask, Recipe, Step,
};
use rikka_types::{Color, Point, Rect};

use super::{Composer, Refusal, refused, stepped};
use crate::colors::Slot;
use crate::paint::serving::objects;
use crate::tool::Tool;

impl Composer<'_> {
    pub(super) fn draw(&mut self, order: DrawOrder) -> Result<(), Refusal> {
        let layer = self.paintable(order.layer.as_ref())?;
        let mask = self.selection_mask()?;
        let mut marks: Vec<Marking> = Vec::new();
        let mut forms: Vec<Form> = Vec::new();
        for (at, item) in order.items.into_iter().enumerate() {
            let kind = kind(&item);
            match item {
                DrawKind::Mark(order) if order.tool.is_freehand() => {
                    marks.extend(self.marks(&order).into_iter().map(Marking::from));
                }
                DrawKind::Brush(order) => {
                    marks.extend(self.brushed(&order).into_iter().map(Marking::from));
                }
                DrawKind::Mark(_) | DrawKind::Path(_) | DrawKind::Text(_) => {
                    let made = self.forms(item);
                    forms.extend(stepped("item", at, kind, made)?);
                }
                other => {
                    self.flush(layer, &mut marks, mask.as_ref())?;
                    let drawn = self.alone(layer, other, mask.as_ref());
                    stepped("item", at, kind, drawn)?;
                }
            }
        }
        self.flush(layer, &mut marks, mask.as_ref())?;
        self.place(layer, forms, AddMethod::Draw)
    }

    pub(super) fn forms(&self, item: DrawKind) -> Result<Vec<Form>, Refusal> {
        match item {
            DrawKind::Mark(order) if !order.tool.is_freehand() => Ok(self.shapes(&order)),
            DrawKind::Path(order) => {
                Ok(vec![Form::Path(order.figure.antialiased(
                    order.antialias.unwrap_or(self.defaults.options.antialiased),
                ))])
            }
            DrawKind::Text(order) => Ok(vec![Form::Text(self.lettering(&order)?)]),
            _ => Err(refused(
                "only lines, arrows, rectangles, ellipses, paths and text are objects",
            )),
        }
    }

    pub(super) fn place(
        &mut self,
        layer: LayerId,
        forms: Vec<Form>,
        by: AddMethod,
    ) -> Result<(), Refusal> {
        if forms.is_empty() {
            return Ok(());
        }
        let ids = self.document.figure_ids();
        let figures: Vec<Figure> = forms
            .into_iter()
            .map(|form| Figure::new(ids.next(), form))
            .collect();
        let made: Vec<FigureId> = figures.iter().map(|figure| figure.id).collect();
        self.apply(&Edit::Figures {
            layer,
            change: FigureEdit::Add { figures, by },
        })?;
        self.describe(&made);
        Ok(())
    }

    fn shapes(&self, order: &MarkOrder) -> Vec<Form> {
        let (colors, options) = (self.defaults.colors, &self.defaults.options);
        let ink = order.color.unwrap_or(colors.primary);
        let width = order
            .width
            .unwrap_or_else(|| options.width(Tool::drawing(order.tool)).unwrap_or(4.0));
        let style = match order.filling {
            Some(filling) => filling.style(colors.secondary),
            None => options.filling.style(colors, Slot::Primary),
        };
        let kind = Tool::drawing(order.tool).shape();
        order
            .strokes
            .iter()
            .filter_map(|stroke| {
                let (from, to) = (*stroke.first()?, *stroke.last()?);
                let form = kind?.spanned(from, to, Pen::new(ink, width), style)?;
                Some(match order.angle {
                    Some(degrees) => {
                        let center = Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
                        form.mapped(
                            Affine::about(center, Affine::rotation(f64::from(degrees))),
                            false,
                        )
                    }
                    None => form,
                })
            })
            .collect()
    }

    fn alone(
        &mut self,
        layer: LayerId,
        item: DrawKind,
        mask: Option<&Mask>,
    ) -> Result<(), Refusal> {
        let colors = self.defaults.colors;
        match item {
            DrawKind::Gradient(order) => {
                let step = Step::new(gradient(&order, colors.primary, colors.secondary));
                let step = match mask {
                    Some(mask) => step.masked(mask.clone()),
                    None => step,
                };
                self.apply(&Edit::Develop {
                    layer,
                    recipe: Recipe::new().then(step),
                })
            }
            DrawKind::Bucket(order) => {
                let flood = self.flood(&order, Sample::Layer, Some(layer))?;
                self.apply(&Edit::Bucket {
                    layer,
                    flood,
                    color: order.color.unwrap_or(colors.primary),
                })
            }
            DrawKind::Fill(color) => self.apply(&Edit::Fill {
                layer,
                color: color.unwrap_or(colors.primary),
            }),
            DrawKind::Erase(background) => self.apply(&Edit::Erase {
                layer,
                background: background.unwrap_or(Color::WHITE),
            }),
            DrawKind::Mark(_) | DrawKind::Brush(_) | DrawKind::Path(_) | DrawKind::Text(_) => {
                Ok(())
            }
        }
    }

    fn flush(
        &mut self,
        layer: LayerId,
        marks: &mut Vec<Marking>,
        mask: Option<&Mask>,
    ) -> Result<(), Refusal> {
        if marks.is_empty() {
            return Ok(());
        }
        let step = Step::new(Marks::new(std::mem::take(marks)));
        let step = match mask {
            Some(mask) => step.masked(mask.clone()),
            None => step,
        };
        self.apply(&Edit::Develop {
            layer,
            recipe: Recipe::new().then(step),
        })
    }

    fn marks(&self, order: &MarkOrder) -> Vec<Mark> {
        let (colors, options) = (self.defaults.colors, &self.defaults.options);
        let tool = order.tool;
        let ink = order.color.unwrap_or(colors.primary);
        let width = order
            .width
            .unwrap_or_else(|| options.width(Tool::drawing(tool)).unwrap_or(4.0))
            / tool.thickness(1.0);
        order
            .strokes
            .iter()
            .filter_map(|stroke| {
                let (first, rest) = stroke.split_first()?;
                let mut mark = Mark::new(tool, ink, width, *first);
                rest.iter().for_each(|point| {
                    mark.extend_to(*point);
                });
                Some(mark)
            })
            .collect()
    }

    fn brushed(&self, order: &BrushOrder) -> Vec<BrushStroke> {
        let (colors, options) = (self.defaults.colors, &self.defaults.options);
        let tool = match order.erases {
            true => Tool::Eraser,
            false => Tool::Paintbrush,
        };
        let size = order
            .size
            .unwrap_or_else(|| options.width(tool).unwrap_or(20.0));
        let configured = options
            .brushing(tool)
            .map(|brushing| brushing.brush(size, options.antialiased));
        let mut brush = configured.unwrap_or_else(|| rikka_image_edit::Brush::new(size));
        brush.hardness = order.hardness.unwrap_or(brush.hardness);
        brush.opacity = order.opacity.unwrap_or(brush.opacity);
        brush.flow = order.flow.unwrap_or(brush.flow);
        brush.antialiased = order.antialias.unwrap_or(brush.antialiased);
        let ink = match order.erases {
            true => Ink::Erase {
                background: order.color.unwrap_or(colors.secondary),
            },
            false => Ink::Paint(order.color.unwrap_or(colors.primary)),
        };
        order
            .strokes
            .iter()
            .filter_map(|stroke| BrushStroke::from_points(brush, ink, stroke.iter().copied()))
            .collect()
    }

    fn lettering(&self, order: &TextOrder) -> Result<Lettering, Refusal> {
        let letters = &self.defaults.options.letters;
        let color = order.color.unwrap_or(self.defaults.colors.primary);
        let mut lettering = letters.lettering(order.text.clone(), (0, 0), color);
        if let Some(font) = &order.font {
            lettering.font = font.clone();
        }
        lettering.size = order.size.unwrap_or(lettering.size);
        lettering.weight = order.weight.unwrap_or(lettering.weight);
        lettering.italic = order.italic.unwrap_or(lettering.italic);
        lettering.align = order.align.unwrap_or(lettering.align);
        lettering.antialias = order.antialias.unwrap_or(lettering.antialias);
        if order.wrap.is_some() {
            lettering.wrap_width = order.wrap;
        }
        let measured = self.document.measure_text(&lettering)?;
        let bounds = measured.ink.unwrap_or(measured.layout);
        let (left, top) = match order.place {
            Placing::At { point, anchor } => {
                let (across, down) = fractions(anchor);
                (
                    point.0 - across * bounds.width(),
                    point.1 - down * bounds.height(),
                )
            }
            Placing::Inside { rect, anchor } => {
                let (across, down) = fractions(anchor);
                (
                    rect[0] + across * (rect[2] - bounds.width()),
                    rect[1] + down * (rect[3] - bounds.height()),
                )
            }
        };
        lettering.origin = (
            (left - bounds.x()).round() as i32,
            (top - bounds.y()).round() as i32,
        );
        Ok(lettering)
    }

    pub(super) fn describe(&mut self, ids: &[FigureId]) {
        let layers = self.document.layers();
        for id in ids {
            let Some(figure) = paint_document::Layer::holding(&layers, *id)
                .and_then(|layer| layer.figures.iter().find(|figure| figure.id == *id))
            else {
                continue;
            };
            let measured = match &figure.form {
                Form::Text(lettering) => self.document.measure_text(lettering).ok(),
                _ => None,
            };
            let layout = measured
                .map(|placed| {
                    let ink = placed
                        .ink
                        .map(|ink| format!("; ink {}", spans(ink)))
                        .unwrap_or_default();
                    format!(
                        "; layout {}{ink}; first baseline y {:.0}",
                        spans(placed.layout),
                        placed.baseline
                    )
                })
                .unwrap_or_default();
            self.notes
                .push(format!("Object {}{layout}.", objects::describe(figure)));
        }
    }
}

fn kind(item: &DrawKind) -> &'static str {
    match item {
        DrawKind::Mark(_) => "mark",
        DrawKind::Brush(_) => "brush",
        DrawKind::Path(_) => "path",
        DrawKind::Gradient(_) => "gradient",
        DrawKind::Text(_) => "text",
        DrawKind::Bucket(_) => "bucket",
        DrawKind::Fill(_) => "fill",
        DrawKind::Erase(_) => "erase",
    }
}

fn gradient(order: &GradientOrder, primary: Color, secondary: Color) -> Gradient {
    let ink = order.ink.unwrap_or(GradientInk::Colors {
        from: primary,
        to: secondary,
    });
    Gradient::new(order.shape, order.from, order.to, ink)
}

fn fractions(anchor: Anchor) -> (f32, f32) {
    match anchor {
        Anchor::TopLeft => (0.0, 0.0),
        Anchor::Top => (0.5, 0.0),
        Anchor::TopRight => (1.0, 0.0),
        Anchor::Left => (0.0, 0.5),
        Anchor::Center => (0.5, 0.5),
        Anchor::Right => (1.0, 0.5),
        Anchor::BottomLeft => (0.0, 1.0),
        Anchor::Bottom => (0.5, 1.0),
        Anchor::BottomRight => (1.0, 1.0),
    }
}

fn spans(rect: Rect) -> String {
    format!(
        "x {:.0}–{:.0}, y {:.0}–{:.0}",
        rect.x(),
        rect.x() + rect.width(),
        rect.y(),
        rect.y() + rect.height()
    )
}
