use rikka_types::{Point, Rect, Size};
use rustc_hash::FxHashMap;

use crate::controls::node_canvas::model::{
    CanvasLink, CanvasNode, FrameArea, FrameKey, NodeGraph, NodeKey, PortAddr, PortSide,
};

pub(crate) const HEADER: f32 = 26.0;
pub(crate) const ROW: f32 = 22.0;
pub(crate) const FOOT: f32 = 6.0;
pub(crate) const PORT: f32 = 10.0;
pub(crate) const FRAME_TITLE: f32 = 26.0;
pub(crate) const FRAME_PADDING: f32 = 18.0;
pub(crate) const GRIP: f32 = 14.0;
pub(crate) const MANY_SPREAD: f32 = PORT * 1.6;
const MANY_STEP: f32 = 6.0;
pub const MIN_ZOOM: f32 = 0.2;
pub const MAX_ZOOM: f32 = 3.0;

const PORT_REACH: f32 = 9.0;
const LINK_REACH: f32 = 5.0;
const LINK_SAMPLES: usize = 24;
const LINK_LEAD: f32 = 48.0;
const FRAMING_MARGIN: f32 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasView {
    pub pan: Point,
    pub zoom: f32,
}

impl Default for CanvasView {
    fn default() -> Self {
        Self {
            pan: Point::ZERO,
            zoom: 1.0,
        }
    }
}

impl CanvasView {
    pub fn to_local(&self, point: Point) -> Point {
        point * self.zoom + self.pan
    }

    pub fn to_graph(&self, local: Point) -> Point {
        (local - self.pan) * (1.0 / self.zoom)
    }

    pub fn zoomed_at(&self, local: Point, factor: f32) -> Self {
        let zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if zoom == self.zoom {
            return *self;
        }
        let anchor = self.to_graph(local);
        Self {
            pan: local - anchor * zoom,
            zoom,
        }
    }

    pub fn panned(&self, by: Point) -> Self {
        Self {
            pan: self.pan + by,
            zoom: self.zoom,
        }
    }

    pub fn framing(area: Rect, room: Size) -> Self {
        let wanted = Size::new(
            area.width() + FRAMING_MARGIN * 2.0,
            area.height() + FRAMING_MARGIN * 2.0,
        );
        let zoom = (room.width / wanted.width.max(1.0))
            .min(room.height / wanted.height.max(1.0))
            .clamp(MIN_ZOOM, 1.0);
        let center = area.center();
        Self {
            pan: Point::new(room.width * 0.5, room.height * 0.5) - center * zoom,
            zoom,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hit {
    Port(PortAddr),
    Node(NodeKey),
    Link(CanvasLink),
    FrameGrip(FrameKey),
    FrameTitle(FrameKey),
    Frame(FrameKey),
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toward {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug)]
struct Placed {
    key: NodeKey,
    rect: Rect,
    detail: bool,
    outputs: usize,
}

#[derive(Clone, Debug)]
struct FramePlace {
    key: FrameKey,
    rect: Rect,
    resizable: bool,
}

#[derive(Clone, Debug)]
pub struct Geometry {
    nodes: Vec<Placed>,
    index: FxHashMap<NodeKey, usize>,
    links: Vec<(CanvasLink, Vec<Point>)>,
    frames: Vec<FramePlace>,
}

pub fn node_height(node: &CanvasNode) -> f32 {
    let detail = usize::from(node.detail.is_some() || node.swatch.is_some());
    HEADER + (detail + node.outputs.len() + node.inputs.len()) as f32 * ROW + FOOT
}

pub(crate) fn port_offset(node: &CanvasNode, side: PortSide, index: usize) -> Point {
    let detail = usize::from(node.detail.is_some() || node.swatch.is_some());
    let row = detail
        + match side {
            PortSide::Output => index,
            PortSide::Input => node.outputs.len() + index,
        };
    let y = HEADER + row as f32 * ROW + ROW * 0.5;
    match side {
        PortSide::Input => Point::new(0.0, y),
        PortSide::Output => Point::new(node.width, y),
    }
}

pub fn link_curve(from: Point, to: Point) -> Vec<Point> {
    let lead = ((to.x - from.x).abs() * 0.5).max(LINK_LEAD);
    let first = Point::new(from.x + lead, from.y);
    let second = Point::new(to.x - lead, to.y);
    (0..=LINK_SAMPLES)
        .map(|step| {
            let t = step as f32 / LINK_SAMPLES as f32;
            let u = 1.0 - t;
            from * (u * u * u)
                + first * (3.0 * u * u * t)
                + second * (3.0 * u * t * t)
                + to * (t * t * t)
        })
        .collect()
}

fn distance_to_segment(point: Point, from: Point, to: Point) -> f32 {
    let along = to - from;
    let length = along.x * along.x + along.y * along.y;
    if length <= f32::EPSILON {
        return point.distance(from);
    }
    let offset = point - from;
    let t = ((offset.x * along.x + offset.y * along.y) / length).clamp(0.0, 1.0);
    point.distance(from + along * t)
}

impl Geometry {
    pub fn new(graph: &NodeGraph) -> Self {
        Self::moved(graph, &[], Point::ZERO)
    }

    pub fn moved(graph: &NodeGraph, moving: &[NodeKey], by: Point) -> Self {
        let mut index = FxHashMap::default();
        let nodes: Vec<Placed> = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(at, node)| {
                index.insert(node.key, at);
                let position = match moving.contains(&node.key) {
                    true => node.position + by,
                    false => node.position,
                };
                Placed {
                    key: node.key,
                    rect: Rect::from_origin_size(
                        position,
                        Size::new(node.width, node_height(node)),
                    ),
                    detail: node.detail.is_some() || node.swatch.is_some(),
                    outputs: node.outputs.len(),
                }
            })
            .collect();
        let mut geometry = Self {
            nodes,
            index,
            links: Vec::new(),
            frames: Vec::new(),
        };
        let mut gathered: FxHashMap<PortAddr, (usize, usize)> = FxHashMap::default();
        for link in &graph.links {
            gathered.entry(link.to).or_insert((0, 0)).1 += 1;
        }
        let many = |port: PortAddr| {
            graph
                .nodes
                .get(*geometry.index.get(&port.node)?)?
                .inputs
                .get(port.index)
                .map(|input| input.many)
        };
        geometry.links = graph
            .links
            .iter()
            .filter_map(|link| {
                let from = geometry.anchor(link.from)?;
                let mut to = geometry.anchor(link.to)?;
                let (rank, count) = gathered.get_mut(&link.to).map_or((0, 1), |slot| {
                    slot.0 += 1;
                    (slot.0 - 1, slot.1)
                });
                if count > 1 && many(link.to) == Some(true) {
                    let step = (MANY_SPREAD / (count - 1) as f32).min(MANY_STEP);
                    to.y += (rank as f32 - (count - 1) as f32 * 0.5) * step;
                }
                Some((*link, link_curve(from, to)))
            })
            .collect();
        geometry.frames = graph
            .frames
            .iter()
            .filter_map(|frame| {
                let (rect, resizable) = match &frame.area {
                    FrameArea::At { position, size } => {
                        (Rect::from_origin_size(*position, *size), true)
                    }
                    FrameArea::Around(members) => {
                        let around = members
                            .iter()
                            .filter_map(|key| geometry.node_rect(*key))
                            .reduce(|one, two| one.union(&two))?;
                        (
                            Rect::new(
                                around.x() - FRAME_PADDING,
                                around.y() - FRAME_PADDING - FRAME_TITLE,
                                around.width() + FRAME_PADDING * 2.0,
                                around.height() + FRAME_PADDING * 2.0 + FRAME_TITLE,
                            ),
                            false,
                        )
                    }
                };
                Some(FramePlace {
                    key: frame.key,
                    rect,
                    resizable,
                })
            })
            .collect();
        geometry
    }

    pub fn node_rect(&self, key: NodeKey) -> Option<Rect> {
        self.index.get(&key).map(|at| self.nodes[*at].rect)
    }

    pub fn frame_rect(&self, key: FrameKey) -> Option<Rect> {
        self.frames
            .iter()
            .find(|frame| frame.key == key)
            .map(|frame| frame.rect)
    }

    pub fn anchor(&self, port: PortAddr) -> Option<Point> {
        let placed = &self.nodes[*self.index.get(&port.node)?];
        let row = usize::from(placed.detail)
            + match port.side {
                PortSide::Output => port.index,
                PortSide::Input => placed.outputs + port.index,
            };
        let y = placed.rect.y() + HEADER + row as f32 * ROW + ROW * 0.5;
        Some(match port.side {
            PortSide::Input => Point::new(placed.rect.x(), y),
            PortSide::Output => Point::new(placed.rect.max_x(), y),
        })
    }

    pub fn link_points(&self) -> impl Iterator<Item = (CanvasLink, &[Point])> {
        self.links
            .iter()
            .map(|(link, points)| (*link, points.as_slice()))
    }

    pub fn bounds(&self) -> Option<Rect> {
        self.nodes
            .iter()
            .map(|node| node.rect)
            .chain(self.frames.iter().map(|frame| frame.rect))
            .reduce(|one, two| one.union(&two))
    }

    pub fn hit(&self, graph: &NodeGraph, view: &CanvasView, local: Point) -> Hit {
        let at = view.to_graph(local);
        let reach = PORT_REACH / view.zoom.min(1.0);
        for (placed, node) in self.nodes.iter().zip(&graph.nodes).rev() {
            if !placed.rect.inflate(reach).contains(at) {
                continue;
            }
            let ports = (0..node.outputs.len())
                .map(|index| PortAddr::output(node.key, index))
                .chain((0..node.inputs.len()).map(|index| PortAddr::input(node.key, index)));
            let nearest = ports
                .filter_map(|port| Some((port, self.anchor(port)?.distance(at))))
                .filter(|(_, away)| *away <= reach)
                .min_by(|one, two| one.1.total_cmp(&two.1));
            if let Some((port, _)) = nearest {
                return Hit::Port(port);
            }
            if placed.rect.contains(at) {
                return Hit::Node(placed.key);
            }
        }
        let link_reach = LINK_REACH / view.zoom;
        let link = self
            .links
            .iter()
            .filter_map(|(link, points)| {
                let away = points
                    .windows(2)
                    .map(|pair| distance_to_segment(at, pair[0], pair[1]))
                    .fold(f32::INFINITY, f32::min);
                (away <= link_reach).then_some((*link, away))
            })
            .min_by(|one, two| one.1.total_cmp(&two.1));
        if let Some((link, _)) = link {
            return Hit::Link(link);
        }
        let grip = GRIP / view.zoom.min(1.0);
        for frame in self.frames.iter().rev() {
            if !frame.rect.contains(at) {
                continue;
            }
            if frame.resizable
                && at.x >= frame.rect.max_x() - grip
                && at.y >= frame.rect.max_y() - grip
            {
                return Hit::FrameGrip(frame.key);
            }
            if at.y <= frame.rect.y() + FRAME_TITLE {
                return Hit::FrameTitle(frame.key);
            }
            return Hit::Frame(frame.key);
        }
        Hit::Empty
    }

    pub fn nodes_within(&self, area: Rect) -> impl Iterator<Item = NodeKey> + '_ {
        self.nodes
            .iter()
            .filter(move |node| node.rect.intersects(&area))
            .map(|node| node.key)
    }

    pub fn frame_members(&self, graph: &NodeGraph, key: FrameKey) -> Vec<NodeKey> {
        let Some(frame) = graph.frames.iter().find(|frame| frame.key == key) else {
            return Vec::new();
        };
        match &frame.area {
            FrameArea::Around(members) => members.clone(),
            FrameArea::At { .. } => Vec::new(),
        }
    }

    pub fn neighbour(&self, from: NodeKey, toward: Toward) -> Option<NodeKey> {
        let start = self.node_rect(from)?.center();
        self.nodes
            .iter()
            .filter(|node| node.key != from)
            .filter_map(|node| {
                let offset = node.rect.center() - start;
                let (along, across) = match toward {
                    Toward::Right => (offset.x, offset.y),
                    Toward::Left => (-offset.x, offset.y),
                    Toward::Down => (offset.y, offset.x),
                    Toward::Up => (-offset.y, offset.x),
                };
                (along > 0.0).then_some((node.key, along + across.abs() * 2.0))
            })
            .min_by(|one, two| one.1.total_cmp(&two.1))
            .map(|(key, _)| key)
    }

    pub fn first(&self) -> Option<NodeKey> {
        self.nodes
            .iter()
            .min_by(|one, two| {
                (one.rect.x() + one.rect.y()).total_cmp(&(two.rect.x() + two.rect.y()))
            })
            .map(|node| node.key)
    }
}
