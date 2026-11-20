use std::collections::HashMap;

use rikka_compositor_desktop::SnapshotId;
use rikka_compositor_geometry::{Logical, Physical, Point, Rect, Scale, Size, Transform};
use rikka_renderer::{Audience, ImageSource, Node, NodeId, Scene};
use rikka_types::{Color, Rect as SceneRect, RoundedRect};

use super::caption::{CaptionNodes, CaptionPaint};
use super::depth::Depth;
use super::edge::EdgeNode;
use super::staging::Leaving;
use super::textures::Look;
use super::tree::{Placement, SurfaceTree, physical};
use super::view::View;
use super::window_shape::{Shaped, WindowShape};
use crate::glass::{Frost, pane_key};

#[derive(Debug, Clone, Copy, PartialEq)]
struct Piece {
    look: Look,
    source: Option<(SceneRect, Transform)>,
    rect: Rect<f64, Logical>,
    opacity: f32,
}

#[derive(Debug)]
pub(crate) struct Frozen {
    pieces: Vec<Piece>,
    shape: Option<WindowShape>,
    caption: Option<CaptionPaint>,
    frost: Frost,
}

impl Frozen {
    pub(crate) fn capture(
        tree: &SurfaceTree,
        view: View,
        geometry: Rect<i32, Logical>,
        frost: Frost,
    ) -> Option<Self> {
        let View { area, scale } = view;
        if scale <= 0.0 {
            return None;
        }
        let corner = (area.loc - geometry.loc).to_f64();
        let pieces: Vec<Piece> = tree
            .drawn()
            .iter()
            .map(|drawn| {
                let (look, source) = drawn.look();
                let rect = drawn.rect;
                Piece {
                    look,
                    source,
                    rect: Rect::new(
                        Point::new(f64::from(rect.loc.x) / scale, f64::from(rect.loc.y) / scale)
                            + corner,
                        Size::new(
                            f64::from(rect.size.w) / scale,
                            f64::from(rect.size.h) / scale,
                        ),
                    ),
                    opacity: drawn.opacity(),
                }
            })
            .collect();
        let caption = tree.caption().map(|placed| CaptionPaint {
            geometry,
            ..placed.paint().clone()
        });
        let band = caption
            .as_ref()
            .filter(|caption| caption.translucent())
            .map(|caption| (pane_key(), caption.band(Point::zero())));
        let frost: Frost = frost.into_iter().chain(band).collect();
        let shape = tree.shape();
        let frost = match shape
            .map(WindowShape::radius)
            .filter(|radius| *radius > 0.0)
        {
            Some(radius) => {
                let size = geometry.size;
                let corners = RoundedRect::new(
                    SceneRect::new(0.0, 0.0, size.w as f32, size.h as f32),
                    radius,
                );
                frost
                    .into_iter()
                    .filter_map(|(key, shape)| shape.intersect(corners).map(|shape| (key, shape)))
                    .collect()
            }
            None => frost,
        };
        (!pieces.is_empty()).then_some(Self {
            pieces,
            shape,
            caption,
            frost,
        })
    }

    pub(crate) fn frost(&self) -> &Frost {
        &self.frost
    }

    pub(crate) fn looks(&self) -> impl Iterator<Item = Look> + '_ {
        self.pieces
            .iter()
            .map(|piece| piece.look)
            .filter(|look| matches!(look, Look::Texture(..)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PlacedNode {
    node: NodeId,
    rect: SceneRect,
    opacity: f32,
    clip: Option<RoundedRect>,
    z: i32,
}

#[derive(Debug, Default)]
struct Placed {
    pieces: Vec<PlacedNode>,
    edge: EdgeNode,
    caption: CaptionNodes,
}

#[derive(Debug, Default)]
pub(crate) struct FrozenNodes {
    drawn: HashMap<SnapshotId, Placed>,
    seen: Vec<SnapshotId>,
}

impl FrozenNodes {
    pub(crate) fn draw(
        &mut self,
        scene: &mut Scene,
        frozen: &Frozen,
        leaving: &Leaving,
        view: View,
        edge: Color,
        depth: &mut Depth,
    ) {
        let Leaving {
            snapshot,
            geometry,
            shown,
        } = *leaving;
        let stretch = shown.stretch(geometry);
        let origin = shown.rect.loc - view.area.loc.to_f64();
        let shaped = frozen.shape.map(|shape| {
            Shaped::new(
                shape,
                physical(origin, shown.rect.size, view.scale.into()),
                (view.scale * stretch.x.min(stretch.y)) as f32,
            )
        });
        let clip = shaped.and_then(|shaped| shaped.clip());
        let mut previous = self.drawn.remove(&snapshot).unwrap_or_default();
        let caption = frozen.caption.clone().map(|caption| {
            let at = caption.geometry.loc;
            caption.place(
                at,
                &Placement {
                    origin: Point::new(origin.x / stretch.x, origin.y / stretch.y),
                    scale: Scale {
                        x: view.scale * stretch.x,
                        y: view.scale * stretch.y,
                    },
                    alpha: shown.alpha,
                    clip,
                },
            )
        });
        previous
            .caption
            .draw(scene, caption, Audience::Everyone, depth);
        let mut placed = Vec::with_capacity(frozen.pieces.len());
        for (index, piece) in frozen.pieces.iter().enumerate() {
            let z = depth.next();
            let at = Point::new(
                origin.x + piece.rect.loc.x * stretch.x,
                origin.y + piece.rect.loc.y * stretch.y,
            );
            let size = Size::new(piece.rect.size.w * stretch.x, piece.rect.size.h * stretch.y);
            let rect = physical(at, size, view.scale.into()).into();
            let opacity = piece.opacity * shown.alpha;
            let next = match previous.pieces.get(index) {
                Some(node) => {
                    if (node.rect, node.opacity, node.clip, node.z) != (rect, opacity, clip, z) {
                        scene
                            .edit(node.node)
                            .rect(rect)
                            .opacity(opacity)
                            .clip(clip)
                            .z(z);
                    }
                    PlacedNode {
                        rect,
                        opacity,
                        clip,
                        z,
                        ..*node
                    }
                }
                None => {
                    let node = node(piece, rect).opacity(opacity).z(z);
                    PlacedNode {
                        node: scene.insert(match clip {
                            Some(clip) => node.clip(clip),
                            None => node,
                        }),
                        rect,
                        opacity,
                        clip,
                        z,
                    }
                }
            };
            placed.push(next);
        }
        for gone in previous.pieces.iter().skip(placed.len()) {
            scene.remove(gone.node);
        }
        previous.edge.draw(
            scene,
            shaped.and_then(|shaped| shaped.edge(edge, shown.alpha)),
            Audience::Everyone,
            depth,
        );
        self.drawn.insert(
            snapshot,
            Placed {
                pieces: placed,
                edge: previous.edge,
                caption: previous.caption,
            },
        );
        self.seen.push(snapshot);
    }

    pub(crate) fn sweep(&mut self, scene: &mut Scene) {
        let seen = std::mem::take(&mut self.seen);
        self.drawn.retain(|snapshot, placed| {
            let visible = seen.contains(snapshot);
            if !visible {
                for gone in placed.pieces.drain(..) {
                    scene.remove(gone.node);
                }
                placed.edge.clear(scene);
                placed.caption.clear(scene);
            }
            visible
        });
    }

    pub(crate) fn drawn_for(
        &self,
        snapshot: SnapshotId,
    ) -> impl Iterator<Item = (usize, Rect<i32, Physical>, f32, Option<RoundedRect>)> + '_ {
        self.drawn.get(&snapshot).into_iter().flat_map(|placed| {
            placed.pieces.iter().enumerate().map(|(index, piece)| {
                (index, Rect::covering(piece.rect), piece.opacity, piece.clip)
            })
        })
    }
}

fn node(piece: &Piece, rect: SceneRect) -> Node {
    match piece.look {
        Look::Texture(texture, color) => {
            let source = piece
                .source
                .map_or_else(ImageSource::whole, |(region, transform)| {
                    ImageSource::whole().region(region).transform(transform)
                });
            Node::image(rect, texture).source(source.color(color))
        }
        Look::Solid(color) => Node::rect(rect).fill(color),
    }
}
