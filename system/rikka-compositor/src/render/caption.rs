use std::sync::Arc;

use rikka_compositor_caption::{Button, Caption};
use rikka_compositor_geometry::{Logical, Physical, Point, Rect, Scale, Size};
use rikka_renderer::{Audience, Node, NodeId, Scene};
use rikka_symbols::MaterialSymbol;
use rikka_text::{IconSpec, TextLayout, TextPaint, TextSpec, Weight, Wrap};
use rikka_theme::caption;
use rikka_types::{Color, Rect as SceneRect, RoundedRect};

use super::depth::Depth;
use super::tree::{Placement, physical};
use crate::glass::pane_key;

const SLOTS: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ButtonPaint {
    pub(crate) button: Button,
    pub(crate) fill: Color,
    pub(crate) glyph: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CaptionPaint {
    pub(crate) caption: Caption,
    pub(crate) geometry: Rect<i32, Logical>,
    pub(crate) title: Arc<str>,
    pub(crate) title_colour: Color,
    pub(crate) title_size: f32,
    pub(crate) fill: Color,
    pub(crate) separator: Color,
    pub(crate) buttons: Vec<ButtonPaint>,
}

impl CaptionPaint {
    pub(crate) fn translucent(&self) -> bool {
        self.fill.a < 1.0
    }

    pub(crate) fn band(&self, offset: Point<f64, Logical>) -> RoundedRect {
        let band = self.caption.band();
        RoundedRect::rect(SceneRect::new(
            (offset.x + band.loc.x) as f32,
            (offset.y + band.loc.y) as f32,
            band.size.w as f32,
            band.size.h as f32,
        ))
    }

    pub(crate) fn place(
        self,
        drawn_at: Point<i32, Logical>,
        placement: &Placement,
    ) -> PlacedCaption {
        PlacedCaption {
            offset: (self.geometry.loc - drawn_at).to_f64(),
            origin: placement.origin,
            scale: placement.scale,
            clip: placement.clip,
            alpha: placement.alpha,
            paint: self,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlacedCaption {
    paint: CaptionPaint,
    offset: Point<f64, Logical>,
    origin: Point<f64, Logical>,
    scale: Scale<f64>,
    clip: Option<RoundedRect>,
    alpha: f32,
}

impl PlacedCaption {
    pub(crate) fn paint(&self) -> &CaptionPaint {
        &self.paint
    }

    pub(crate) fn alpha(&self) -> f32 {
        self.alpha
    }

    pub(crate) fn with_alpha(mut self, alpha: f32) -> Self {
        self.alpha *= alpha;
        self
    }

    pub(crate) fn band(&self) -> RoundedRect {
        self.paint.band(self.offset)
    }

    fn rect(&self, local: Rect<f64, Logical>) -> Rect<i32, Physical> {
        physical(
            self.origin + self.offset + local.loc,
            local.size,
            self.scale,
        )
    }

    fn zoom(&self) -> f32 {
        self.scale.x.min(self.scale.y) as f32
    }

    pub(crate) fn area(&self) -> Rect<i32, Physical> {
        self.rect(self.paint.caption.band())
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Look {
    Fill,
    Text(TextPaint),
    Icon(IconSpec),
}

impl Look {
    fn alike(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Piece {
    rect: Rect<i32, Physical>,
    colour: Color,
    look: Look,
}

impl Piece {
    fn node(&self) -> Node {
        let rect = SceneRect::from(self.rect);
        let node = match &self.look {
            Look::Fill => Node::rect(rect),
            Look::Text(paint) => Node::text(rect, paint.clone()),
            Look::Icon(spec) => Node::icon(rect, *spec),
        };
        node.fill(self.colour)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Layer {
    z: i32,
    audience: Audience,
    clip: Option<RoundedRect>,
    alpha: f32,
}

#[derive(Debug)]
struct Drawn {
    placed: PlacedCaption,
    layer: Layer,
    pieces: Vec<(NodeId, Piece)>,
}

#[derive(Debug)]
struct TitleLayout {
    title: Arc<str>,
    size: f32,
    width: f32,
    layout: Arc<TextLayout>,
}

#[derive(Debug, Default)]
pub(crate) struct CaptionNodes {
    drawn: Option<Drawn>,
    title: Option<TitleLayout>,
    changed: Vec<Rect<i32, Physical>>,
    key: Option<u64>,
}

impl CaptionNodes {
    pub(crate) fn placed(&self) -> Option<&PlacedCaption> {
        self.drawn.as_ref().map(|drawn| &drawn.placed)
    }

    pub(crate) fn changed(&self) -> &[Rect<i32, Physical>] {
        &self.changed
    }

    pub(crate) fn key(&mut self) -> u64 {
        *self.key.get_or_insert_with(pane_key)
    }

    pub(crate) fn draw(
        &mut self,
        scene: &mut Scene,
        placed: Option<PlacedCaption>,
        audience: Audience,
        depth: &mut Depth,
    ) {
        self.changed.clear();
        let Some(placed) = placed else {
            return self.clear(scene);
        };
        let layer = Layer {
            z: depth.reserve(SLOTS),
            audience,
            clip: placed.clip,
            alpha: placed.alpha,
        };
        if let Some(drawn) = &self.drawn
            && (&drawn.placed, drawn.layer) == (&placed, layer)
        {
            return;
        }
        let pieces = self.pieces(&placed);
        let pieces = match self.drawn.take() {
            Some(drawn) if alike(&drawn.pieces, &pieces) => {
                self.restyle(scene, drawn, pieces, layer)
            }
            Some(drawn) => {
                self.remove(scene, drawn);
                self.insert(scene, pieces, layer)
            }
            None => self.insert(scene, pieces, layer),
        };
        self.drawn = Some(Drawn {
            placed,
            layer,
            pieces,
        });
    }

    pub(crate) fn clear(&mut self, scene: &mut Scene) {
        if let Some(drawn) = self.drawn.take() {
            self.remove(scene, drawn);
        }
    }

    fn insert(
        &mut self,
        scene: &mut Scene,
        pieces: Vec<Piece>,
        layer: Layer,
    ) -> Vec<(NodeId, Piece)> {
        pieces
            .into_iter()
            .zip(layer.z + 1..)
            .map(|(piece, z)| {
                let node = piece
                    .node()
                    .z(z)
                    .audience(layer.audience)
                    .opacity(layer.alpha);
                let node = match layer.clip {
                    Some(clip) => node.clip(clip),
                    None => node,
                };
                self.changed.push(piece.rect);
                (scene.insert(node), piece)
            })
            .collect()
    }

    fn restyle(
        &mut self,
        scene: &mut Scene,
        drawn: Drawn,
        pieces: Vec<Piece>,
        layer: Layer,
    ) -> Vec<(NodeId, Piece)> {
        let was = drawn.layer;
        drawn
            .pieces
            .into_iter()
            .zip(pieces)
            .zip(layer.z + 1..)
            .map(|(((node, old), new), z)| {
                let mut edit = scene.edit(node);
                if old.rect != new.rect {
                    edit.rect(SceneRect::from(new.rect));
                }
                if old.colour != new.colour {
                    edit.fill(new.colour);
                }
                match &new.look {
                    Look::Text(paint) if old.look != new.look => {
                        edit.text(paint.clone());
                    }
                    Look::Icon(spec) if old.look != new.look => {
                        edit.icon(*spec);
                    }
                    Look::Fill | Look::Text(_) | Look::Icon(_) => {}
                }
                if was.z != layer.z {
                    edit.z(z);
                }
                if was.audience != layer.audience {
                    edit.audience(layer.audience);
                }
                if was.clip != layer.clip {
                    edit.clip(layer.clip);
                }
                if was.alpha != layer.alpha {
                    edit.opacity(layer.alpha);
                }
                drop(edit);
                if old != new || was != layer {
                    self.changed.extend([old.rect, new.rect]);
                }
                (node, new)
            })
            .collect()
    }

    fn remove(&mut self, scene: &mut Scene, drawn: Drawn) {
        for (node, piece) in drawn.pieces {
            self.changed.push(piece.rect);
            scene.remove(node);
        }
    }

    fn pieces(&mut self, placed: &PlacedCaption) -> Vec<Piece> {
        let paint = &placed.paint;
        let caption = &paint.caption;
        let zoom = placed.zoom();
        let mut pieces = vec![
            Piece {
                rect: placed.rect(caption.bar()),
                colour: paint.fill,
                look: Look::Fill,
            },
            Piece {
                rect: placed.rect(caption.separator()),
                colour: paint.separator,
                look: Look::Fill,
            },
        ];
        let title = caption.title();
        if !paint.title.is_empty() && title.size.w > 0.0 {
            let layout = self.title_layout(paint, title.size.w as f32);
            let line = f64::from(layout.line_height());
            let middle = title.loc.y + title.size.h / 2.0;
            let row = Rect::new(
                Point::new(title.loc.x, middle - line / 2.0),
                Size::new(title.size.w, line),
            );
            pieces.push(Piece {
                rect: placed.rect(row),
                colour: paint.title_colour,
                look: Look::Text(TextPaint::new(layout).scaled(zoom)),
            });
        }
        for ((button, rect), look) in caption.buttons().zip(&paint.buttons) {
            let shown = placed.rect(rect);
            let glyph = IconSpec::new(symbol(button).codepoint(), caption::GLYPH)
                .weight(Weight::LIGHT)
                .scaled(zoom);
            pieces.push(Piece {
                rect: shown,
                colour: look.fill,
                look: Look::Fill,
            });
            pieces.push(Piece {
                rect: shown,
                colour: look.glyph,
                look: Look::Icon(glyph),
            });
        }
        pieces
    }

    fn title_layout(&mut self, paint: &CaptionPaint, width: f32) -> Arc<TextLayout> {
        if let Some(laid) = &self.title
            && laid.title == paint.title
            && laid.size == paint.title_size
            && laid.width == width
        {
            return laid.layout.clone();
        }
        let layout = TextSpec::new(paint.title.clone(), paint.title_size)
            .weight(Weight::MEDIUM)
            .wrap(Wrap::None)
            .ellipsize(true)
            .layout(Some(width));
        self.title = Some(TitleLayout {
            title: paint.title.clone(),
            size: paint.title_size,
            width,
            layout: layout.clone(),
        });
        layout
    }
}

fn symbol(button: Button) -> MaterialSymbol {
    match button {
        Button::Minimize => MaterialSymbol::REMOVE,
        Button::Maximize => MaterialSymbol::CROP_SQUARE,
        Button::Restore => MaterialSymbol::FILTER_NONE,
        Button::Close => MaterialSymbol::CLOSE,
    }
}

fn alike(drawn: &[(NodeId, Piece)], pieces: &[Piece]) -> bool {
    drawn.len() == pieces.len()
        && drawn
            .iter()
            .zip(pieces)
            .all(|((_, old), new)| old.look.alike(&new.look))
}
